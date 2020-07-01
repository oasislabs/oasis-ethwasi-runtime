//! Test client to interact with an oasis-ethwasi-runtime blockchain.
use std::{collections::HashMap, str::FromStr, sync::Arc};

use byteorder::{BigEndian, ByteOrder};
use elastic_array::ElasticArray128;
use ethcore::{
    executive::contract_address,
    rlp,
    transaction::{Action, Transaction as EthcoreTransaction},
    vm::{ConfidentialCtx as EthConfidentialCtx, OASIS_HEADER_PREFIX},
};
use ethereum_types::{Address, H256, U256};
use ethkey::{KeyPair as EtyKeyPair, Secret};
use oasis_core_keymanager_client::{self, KeyManagerClient, KeyPair, KeyPairId};
use oasis_core_runtime::{
    common::{
        crypto::{
            hash::Hash,
            mrae::nonce::{Nonce, NONCE_SIZE},
        },
        roothash::Header,
    },
    executor::Executor,
    runtime_context,
    storage::{
        mkvs::{sync::NoopReadSyncer, Tree},
        StorageContext,
    },
    transaction::{dispatcher::BatchHandler, Context as TxnContext},
};

use io_context::Context as IoContext;
use keccak_hash::keccak;
use oasis_ethwasi_runtime_api::ExecutionResult;
use oasis_ethwasi_runtime_common::{
    confidential::ConfidentialCtx,
    genesis,
    parity::NullBackend,
    storage::{MemoryKeyValue, ThreadLocalMKVS},
};
use serde_json::map::Map;

use crate::{
    block::{BlockContext, OasisBatchHandler},
    dispatcher, methods,
};

/// Test client.
pub struct Client {
    /// KeyPair used for signing transactions.
    pub keypair: EtyKeyPair,
    /// The client's keys used for generating the encrypted `data` field to
    /// send transactions from Client -> Enclave.
    pub ephemeral_key: KeyPair,
    /// Gas limit used for transactions.
    /// TODO: use estimate gas to set this dynamically
    pub gas_limit: U256,
    /// Gas price used for transactions.
    pub gas_price: U256,
    /// Header.
    pub header: Header,
    /// In-memory MKVS.
    pub mkvs: Option<Tree>,
    /// Key manager client.
    pub km_client: Arc<dyn KeyManagerClient>,
    /// Results.
    pub results: HashMap<H256, ExecutionResult>,
}

impl Client {
    pub fn new() -> Self {
        let km_client = Arc::new(oasis_core_keymanager_client::mock::MockClient::new());
        let mut mkvs = Tree::make().new(Box::new(NoopReadSyncer {}));

        // Initialize genesis.
        let untrusted_local = Arc::new(MemoryKeyValue::new());
        StorageContext::enter(&mut mkvs, untrusted_local, || {
            genesis::SPEC
                .ensure_db_good(
                    Box::new(ThreadLocalMKVS::new(IoContext::background())),
                    NullBackend,
                    &Default::default(),
                )
                .expect("genesis initialization must succeed");
        });

        let (_, state_root) = mkvs
            .commit(IoContext::background(), Default::default(), 0)
            .expect("mkvs commit must succeed");

        Self {
            // address: 0x7110316b618d20d0c44728ac2a3d683536ea682
            keypair: EtyKeyPair::from_secret(
                Secret::from_str(
                    "533d62aea9bbcb821dfdda14966bb01bfbbb53b7e9f5f0d69b8326e052e3450c",
                )
                .unwrap(),
            )
            .unwrap(),
            ephemeral_key: KeyPair::generate_mock(),
            gas_price: U256::from(1000000000),
            gas_limit: U256::from(1000000),
            mkvs: Some(mkvs),
            km_client,
            header: Header {
                round: 0,
                previous_hash: Hash::empty_hash(),
                timestamp: 0xcafedeadbeefc0de,
                state_root,
                ..Default::default()
            },
            results: HashMap::new(),
        }
    }

    pub fn check_batch<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self, &mut TxnContext) -> R,
    {
        let mut mkvs = self.mkvs.take().expect("nested execute_batch not allowed");
        let header = self.header.clone();
        let mut ctx = TxnContext::new(IoContext::background().freeze(), &header, true);
        let handler = OasisBatchHandler::new(self.km_client.clone());
        let untrusted_local = Arc::new(MemoryKeyValue::new());

        let result = StorageContext::enter(&mut mkvs, untrusted_local, || {
            handler.start_batch(&mut ctx);
            let result = f(self, &mut ctx);
            handler.end_batch(&mut ctx);

            result
        });
        self.mkvs = Some(mkvs);

        result
    }

    pub fn execute_batch<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self, &mut TxnContext) -> R,
    {
        let mut mkvs = self.mkvs.take().expect("nested execute_batch not allowed");
        let header = self.header.clone();
        let mut ctx = TxnContext::new(IoContext::background().freeze(), &header, false);
        let handler = OasisBatchHandler::new(self.km_client.clone());
        let untrusted_local = Arc::new(MemoryKeyValue::new());

        let result = StorageContext::enter(&mut mkvs, untrusted_local, || {
            handler.start_batch(&mut ctx);
            let result = f(self, &mut ctx);
            handler.end_batch(&mut ctx);

            result
        });

        let (_, new_state_root) = mkvs
            .commit(
                IoContext::background(),
                Default::default(),
                self.header.round + 1,
            )
            .expect("mkvs commit must succeed");
        self.header.state_root = new_state_root;
        // Just want a deterministic, random-looking value for block hash.
        self.header.previous_hash = Hash::digest_bytes(self.header.previous_hash.as_ref());
        self.header.round += 1;
        self.mkvs = Some(mkvs);

        result
    }

    /// Sets the timestamp passed to the runtime.
    pub fn set_timestamp(&mut self, timestamp: u64) {
        self.header.timestamp = timestamp;
    }

    /// Returns an encrypted form of the data field to be used in a web3c confidential
    /// transaction
    pub fn confidential_data(&self, contract: Option<&Address>, data: Vec<u8>) -> Vec<u8> {
        if contract.is_none() {
            // Don't encrypt confidential deploys.
            let mut conf_deploy_data = Self::make_header(None, Some(true));
            conf_deploy_data.append(&mut data.clone());
            return conf_deploy_data;
        }

        let contract_addr = contract.unwrap();
        let enc_data = self
            .client_confidential_ctx(contract_addr.clone())
            .encrypt_session(data)
            .unwrap();

        enc_data
    }

    /// Creates a non-confidential contract, return the transaction hash for the deploy
    /// and the address of the contract.
    pub fn create_contract(&mut self, code: Vec<u8>, balance: &U256) -> (H256, Address) {
        let (hash, address) = self
            .send(None, code, balance, None)
            .expect("deployment should succeed");
        (hash, address.unwrap())
    }

    /// Creates a contract with specified expiry and confidentiality, returns the
    /// transaction hash for the deploy and the address of the contract.
    pub fn create_contract_with_header(
        &mut self,
        code: Vec<u8>,
        balance: &U256,
        expiry: Option<u64>,
        confidentiality: Option<bool>,
    ) -> (H256, Address) {
        let mut data = Self::make_header(expiry, confidentiality);
        data.extend(code);
        let (hash, address) = self
            .send(None, data, balance, None)
            .expect("deployment should succeed");
        (hash, address.unwrap())
    }

    /// Returns the receipt for the given transaction hash.
    pub fn result(&mut self, tx_hash: H256) -> ExecutionResult {
        self.results.get(&tx_hash).unwrap().clone()
    }

    pub fn nonce(&mut self, address: &Address) -> U256 {
        self.execute_batch(|_client, ctx| {
            let ectx = runtime_context!(ctx, BlockContext);
            ectx.state.nonce(address)
        })
        .unwrap()
    }

    pub fn balance(&mut self, address: &Address) -> U256 {
        self.execute_batch(|_client, ctx| {
            let ectx = runtime_context!(ctx, BlockContext);
            ectx.state.balance(address)
        })
        .unwrap()
    }

    /// Returns the transaction hash and address of the confidential contract. The code given
    /// should not have the confidential prefix, as that will be added automatically.
    pub fn create_confidential_contract(
        &mut self,
        code: Vec<u8>,
        balance: &U256,
    ) -> (H256, Address) {
        let (hash, address) = self.confidential_send(None, code, balance);
        (hash, address.unwrap())
    }

    /// Returns the return value of the contract's method.
    pub fn call(&mut self, contract: &Address, data: Vec<u8>, value: &U256) -> Vec<u8> {
        let (hash, _) = self
            .send(Some(contract), data, value, None)
            .expect("call should succeed");
        let result = self.result(hash);
        result.output
    }

    /// Sends a transaction onchain that updates the blockchain, analagous to the web3.js send().
    pub fn send(
        &mut self,
        contract: Option<&Address>,
        data: Vec<u8>,
        value: &U256,
        nonce: Option<U256>,
    ) -> Result<(H256, Option<Address>), String> {
        self.execute_batch(|client, ctx| {
            let ectx = runtime_context!(ctx, BlockContext);
            let tx = EthcoreTransaction {
                action: if contract == None {
                    Action::Create
                } else {
                    Action::Call(*contract.unwrap())
                },
                nonce: nonce.unwrap_or(ectx.state.nonce(&client.keypair.address()).unwrap()),
                gas_price: client.gas_price,
                gas: client.gas_limit,
                value: *value,
                data: data,
            }
            .sign(&client.keypair.secret(), None);

            let raw = rlp::encode(&tx);
            let decoded_call = dispatcher::DecodedCall {
                transaction: methods::check::tx(&raw, ctx).map_err(|err| err.to_string())?,
            };
            let result = methods::execute::tx(&decoded_call, ctx).map_err(|err| err.to_string())?;
            client.results.insert(tx.hash(), result);

            let address = if contract == None {
                Some(
                    contract_address(
                        genesis::SPEC.engine.create_address_scheme(ctx.header.round),
                        &tx.sender(),
                        &tx.nonce,
                        &tx.data,
                    )
                    .0,
                )
            } else {
                None
            };

            Ok((tx.hash(), address))
        })
    }

    /// Performs a confidential transaction updating the state of the blockchain.
    /// `Data` should be unencrypted (and without a confidential prefix for deploys).
    /// Such details will be added to the transaction automatically. Returns the
    /// transaction's hash.
    pub fn confidential_send(
        &mut self,
        contract: Option<&Address>,
        data: Vec<u8>,
        value: &U256,
    ) -> (H256, Option<Address>) {
        let enc_data = self.confidential_data(contract.clone(), data);
        self.send(contract, enc_data, value, None)
            .expect("confidential send should succeed")
    }

    /// Performs a confidential call, i.e., a simulated transaction that doesn't update
    /// blockchain state. Returns the return value of the contract's functions.
    pub fn confidential_call(
        &mut self,
        contract: &Address,
        data: Vec<u8>,
        value: &U256,
    ) -> Vec<u8> {
        let enc_data = self.confidential_data(Some(contract), data);
        let (hash, _) = self
            .send(Some(contract), enc_data, value, None)
            .expect("confidential call should succeed");
        let result = self.result(hash);
        self.client_confidential_ctx(*contract)
            .decrypt(result.output)
            .unwrap()
    }

    /// Returns an *active* confidential context used from the perspective of the client,
    /// so that it can encrypt/decrypt transactions to/from web3c.
    ///
    /// In production, a `ConfidentialCtx` will never be created like this. This
    /// is just a convenience to generate the encrypted `data` field to send txs
    /// from Client -> Enclave while testing.
    ///
    /// In addition, this should not be injected into the parity State, because such a
    /// confidential context should be from the perspective of the keymanager.
    ///
    /// See `key_manager_confidential_ctx`, which supplies the dual encryption ctx,
    /// i.e., everything encrypted from `client_confidential_ctx` can be decrypted from
    /// `key_manager_confidential_ctx` and vice versa.
    pub fn client_confidential_ctx(&self, contract: Address) -> ConfidentialCtx {
        let contract_id = KeyPairId::from(&keccak(contract.to_vec())[..]);
        let mut executor = Executor::new();
        let contract_key = executor
            .block_on(
                self.km_client
                    .get_or_create_keys(IoContext::background(), contract_id),
            )
            .unwrap();

        // No need to save the Nonce on the Client (for now).
        let nonce = Nonce::new([0; NONCE_SIZE]);
        ConfidentialCtx::new_test(
            Some(contract_key.input_keypair.get_pk()),
            Some((contract, self.ephemeral_key.clone())),
            Some(nonce),
            true,
            Default::default(),
            // Not to be used for storage encryption, so no need for a Deoxys-II instance
            // or storage nonce.
            None,
            None,
            self.km_client.clone(),
            IoContext::background().freeze(),
        )
    }

    /// Returns an *active* confidential context. Using this with a parity State object will
    /// transparently encrypt/decrypt everything going into and out of contract storage.
    /// Do not use this if you're trying to access *unencrypted* state.
    pub fn key_manager_confidential_ctx(&self, contract: Address) -> ConfidentialCtx {
        let mut ctx = ConfidentialCtx::new(
            self.header.previous_hash.as_ref().into(),
            IoContext::background().freeze(),
            self.km_client.clone(),
        );
        ctx.activate(Some(contract))
            .expect("ConfidentialCtx activate must succeed");
        ctx
    }

    /// Returns the raw underlying storage for the given `contract`--without
    /// encrypting the key or decrypting the return value.
    pub fn raw_storage(&mut self, contract: Address, storage_key: H256) -> Option<Vec<u8>> {
        self.execute_batch(|_client, ctx| {
            let ectx = runtime_context!(ctx, BlockContext);
            ectx.state._storage_at(&contract, &storage_key)
        })
        .unwrap()
    }

    /// Returns the key that actually stores the confidential contract's storage value.
    /// To be used together with `Client::raw_storage`.
    pub fn confidential_storage_key(&self, contract: Address, storage_key: H256) -> H256 {
        let km_confidential_ctx = self.key_manager_confidential_ctx(contract);
        keccak(
            &km_confidential_ctx
                .encrypt_storage_key(storage_key.to_vec())
                .unwrap(),
        )
    }

    /// Returns the storage expiry timestamp for a contract.
    pub fn storage_expiry(&mut self, contract: Address) -> u64 {
        self.execute_batch(|_client, ctx| {
            let ectx = runtime_context!(ctx, BlockContext);
            ectx.state.storage_expiry(&contract)
        })
        .unwrap()
    }

    /// Returns a valid contract deployment header with specified expiry and confidentiality.
    fn make_header(expiry: Option<u64>, confidential: Option<bool>) -> Vec<u8> {
        // start with header prefix
        let mut data = ElasticArray128::from_slice(&OASIS_HEADER_PREFIX[..]);

        // header version 1
        let mut version = [0u8; 2];
        BigEndian::write_u16(&mut version, 1 as u16);

        // contents (JSON)
        let mut map = Map::new();
        confidential
            .map(|confidential| map.insert("confidential".to_string(), confidential.into()));
        expiry.map(|expiry| map.insert("expiry".to_string(), expiry.into()));
        let contents = json!(map).to_string().into_bytes();

        // contents length
        let mut length = [0u8; 2];
        BigEndian::write_u16(&mut length, contents.len() as u16);

        // append header version, length and contents
        data.append_slice(&version);
        data.append_slice(&length);
        data.append_slice(&contents);

        data.into_vec()
    }
}
