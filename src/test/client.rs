//! Test client to interact with a runtime-ethereum blockchain.
use std::{str::FromStr, sync::Arc};

use byteorder::{BigEndian, ByteOrder};
use ekiden_keymanager_client::{self, ContractId, ContractKey, KeyManagerClient};
use ekiden_runtime::{
    common::{
        crypto::{
            hash::Hash,
            mrae::nonce::{Nonce, NONCE_SIZE},
        },
        roothash::Header,
    },
    executor::Executor,
    storage::{cas::MemoryCAS, mkvs::CASPatriciaTrie, StorageContext, CAS, MKVS},
    transaction::{dispatcher::BatchHandler, Context as TxnContext},
};
use elastic_array::ElasticArray128;
use ethcore::{
    rlp,
    transaction::{Action, Transaction as EthcoreTransaction},
    vm::{ConfidentialCtx as EthConfidentialCtx, OASIS_HEADER_PREFIX},
};
use ethereum_types::{Address, H256, U256};
use ethkey::{KeyPair, Secret};

use io_context::Context as IoContext;
use keccak_hash::keccak;
use runtime_ethereum_api::{Receipt, TransactionRequest};
use runtime_ethereum_common::confidential::ConfidentialCtx;
use serde_json::map::Map;

use crate::{cache::Cache, methods, EthereumBatchHandler};

/// Test client.
pub struct Client {
    /// KeyPair used for signing transactions.
    pub keypair: KeyPair,
    /// Contract key used for encrypting web3c transactions.
    pub ephemeral_key: ContractKey,
    /// Gas limit used for transactions.
    /// TODO: use estimate gas to set this dynamically
    pub gas_limit: U256,
    /// Gas price used for transactions.
    pub gas_price: U256,
    /// Header.
    pub header: Header,
    /// In-memory CAS.
    pub cas: Arc<CAS>,
    /// Key manager client.
    pub km_client: Arc<KeyManagerClient>,
    /// State cache.
    pub cache: Arc<Cache>,
}

impl Client {
    pub fn new() -> Self {
        let km_client = Arc::new(ekiden_keymanager_client::mock::MockClient::new());

        Self {
            // address: 0x7110316b618d20d0c44728ac2a3d683536ea682
            keypair: KeyPair::from_secret(
                Secret::from_str(
                    "533d62aea9bbcb821dfdda14966bb01bfbbb53b7e9f5f0d69b8326e052e3450c",
                )
                .unwrap(),
            )
            .unwrap(),
            ephemeral_key: ContractKey::generate(),
            gas_price: U256::from(1000000000),
            gas_limit: U256::from(1000000),
            cas: Arc::new(MemoryCAS::new()),
            cache: Arc::new(Cache::new(km_client.clone())),
            km_client,
            header: Header {
                timestamp: 0xcafedeadbeefc0de,
                state_root: Hash::empty_hash(),
            },
        }
    }

    pub fn execute_batch<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self, &mut TxnContext) -> R,
    {
        println!("execute with header: {:?}", self.header);
        let mut mkvs = CASPatriciaTrie::new(self.cas.clone(), &self.header.state_root);
        let mut ctx = TxnContext::new(IoContext::background().freeze(), self.header.clone());
        let handler = EthereumBatchHandler::new(self.cache.clone());

        let result = StorageContext::enter(self.cas.clone(), &mut mkvs, || {
            handler.start_batch(&mut ctx);
            let result = f(self, &mut ctx);
            handler.end_batch(ctx);

            result
        });

        let new_state_root = mkvs.commit().expect("mkvs commit must succeed");
        self.cache.finalize_root(new_state_root);
        self.header.state_root = new_state_root;

        result
    }

    /// Sets the timestamp passed to the runtime.
    pub fn set_timestamp(&mut self, timestamp: u64) {
        self.header.timestamp = timestamp;
    }

    pub fn estimate_gas(
        &mut self,
        contract: Option<&Address>,
        data: Vec<u8>,
        value: &U256,
    ) -> U256 {
        let tx = TransactionRequest {
            caller: Some(self.keypair.address()),
            is_call: contract.is_some(),
            address: contract.map(|c| *c),
            input: Some(data),
            value: Some(*value),
            nonce: None,
            gas: None,
        };

        self.execute_batch(|_client, ctx| methods::estimate_gas(&tx, ctx).unwrap())
    }

    pub fn confidential_estimate_gas(
        &mut self,
        contract: Option<&Address>,
        data: Vec<u8>,
        value: &U256,
    ) -> U256 {
        self.estimate_gas(contract, self.confidential_data(contract, data), value)
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
            .confidential_ctx(contract_addr.clone())
            .encrypt_session(data)
            .unwrap();

        enc_data
    }

    /// Creates a non-confidential contract, return the transaction hash for the deploy
    /// and the address of the contract.
    pub fn create_contract(&mut self, code: Vec<u8>, balance: &U256) -> (H256, Address) {
        let hash = self.send(None, code, balance);
        let receipt =
            self.execute_batch(|_client, ctx| methods::get_receipt(&hash, ctx).unwrap().unwrap());
        (hash, receipt.contract_address.unwrap())
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
        let hash = self.send(None, data, balance);
        let receipt =
            self.execute_batch(|_client, ctx| methods::get_receipt(&hash, ctx).unwrap().unwrap());
        (hash, receipt.contract_address.unwrap())
    }

    /// Returns the receipt for the given transaction hash.
    pub fn receipt(&mut self, tx_hash: H256) -> Receipt {
        self.execute_batch(|_client, ctx| methods::get_receipt(&tx_hash, ctx))
            .unwrap()
            .unwrap()
    }

    /// Returns the transaction hash and address of the confidential contract. The code given
    /// should not have the confidential prefix, as that will be added automatically.
    pub fn create_confidential_contract(
        &mut self,
        code: Vec<u8>,
        balance: &U256,
    ) -> (H256, Address) {
        let hash = self.confidential_send(None, code, balance);
        let receipt = self
            .execute_batch(|_client, ctx| methods::get_receipt(&hash, ctx))
            .unwrap()
            .unwrap();
        (hash, receipt.contract_address.unwrap())
    }

    /// Makes a simulated transaction, analagous to the web3.js call().
    /// Returns the return value of the contract's method.
    pub fn call(&mut self, contract: &Address, data: Vec<u8>, value: &U256) -> Vec<u8> {
        let tx = TransactionRequest {
            caller: Some(self.keypair.address()),
            is_call: true,
            address: Some(*contract),
            input: Some(data),
            value: Some(*value),
            nonce: None,
            gas: None,
        };

        self.execute_batch(|_client, ctx| methods::simulate_transaction(&tx, ctx))
            .unwrap()
            .result
            .unwrap()
    }

    /// Sends a transaction onchain that updates the blockchain, analagous to the web3.js send().
    pub fn send(&mut self, contract: Option<&Address>, data: Vec<u8>, value: &U256) -> H256 {
        self.execute_batch(|client, ctx| {
            let tx = EthcoreTransaction {
                action: if contract == None {
                    Action::Create
                } else {
                    Action::Call(*contract.unwrap())
                },
                nonce: methods::get_account_nonce(&client.keypair.address(), ctx).unwrap(),
                gas_price: client.gas_price,
                gas: client.gas_limit,
                value: *value,
                data: data,
            }
            .sign(&client.keypair.secret(), None);

            let raw = rlp::encode(&tx);
            methods::execute_raw_transaction(&raw.into_vec(), ctx)
                .unwrap()
                .hash
                .unwrap()
        })
    }

    /// Performs a confidential call, i.e., a simulated transaction that doesn't update
    /// blockchaian state. Returns the return value of the contract's functions.
    pub fn confidential_call(
        &mut self,
        contract: &Address,
        data: Vec<u8>,
        value: &U256,
    ) -> Vec<u8> {
        self.confidential_invocation(Some(contract), data, value, false)
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
    ) -> H256 {
        let tx_hash = self.confidential_invocation(contract, data, value, true);
        assert!(tx_hash.len() == 32);
        H256::from(tx_hash.as_slice())
    }

    /// Performs confidential calls, sends, and deploys.
    fn confidential_invocation(
        &mut self,
        contract: Option<&Address>,
        data: Vec<u8>,
        value: &U256,
        is_send: bool,
    ) -> Vec<u8> {
        let enc_data = self.confidential_data(contract.clone(), data);
        if is_send {
            self.send(contract, enc_data, value).to_vec()
        } else {
            let contract_addr = contract.unwrap();
            let encrypted_result = self.call(contract_addr, enc_data, value);
            self.confidential_ctx(*contract_addr)
                .decrypt(encrypted_result)
                .unwrap()
        }
    }

    /// Returns an *open* confidential context used from the perspective of the client,
    /// so that it can encrypt/decrypt transactions to/from web3c. This should not be
    /// injected into the parity State, because such a confidential context should be
    /// from the perspective of the keymanager. See `key_manager_confidential_ctx`.
    pub fn confidential_ctx(&self, contract: Address) -> ConfidentialCtx {
        self.make_ctx(contract, false)
    }

    /// Returns an *open* confidential context. Using this with a parity State object will
    /// transparently encrypt/decrypt everything going into and out of contract storage.
    /// Do not use this if you're trying to access *unencrypted* state.
    pub fn key_manager_confidential_ctx(&self, contract: Address) -> ConfidentialCtx {
        self.make_ctx(contract, true)
    }

    /// Returns a new, open ConfidentialCtx. Here we use such a context in two ways: 1)
    /// from the "perspective" of the client and 2) from the perspective of the key manager,
    /// i.e., a contract execution inside an enclave. The former can be used to encrypt/decrypt
    /// to web3c. The latter can be used to encrypt/decrypt inside web3c (just as a compute node
    /// would).
    fn make_ctx(&self, contract: Address, is_key_manager: bool) -> ConfidentialCtx {
        let contract_id = ContractId::from(&keccak(contract.to_vec())[..]);
        let mut executor = Executor::new();
        let contract_key = executor
            .block_on(
                self.km_client
                    .get_or_create_keys(IoContext::background(), contract_id),
            )
            .unwrap();

        // Note that what key is used as the "peer" switches depending upon `is_key_manager`.
        // From the perspective of the client, the "peer" is the contract (i.e. the key
        // manager), and vice versa. This is a result of our mrae's symmetric key derivation.
        let (peer_key, contract_key) = if is_key_manager {
            (self.ephemeral_key.input_keypair.get_pk(), contract_key)
        } else {
            (
                contract_key.input_keypair.get_pk(),
                self.ephemeral_key.clone(),
            )
        };
        // No need to save the Nonce on the Client (for now).
        let nonce = Nonce::new([0; NONCE_SIZE]);
        ConfidentialCtx {
            peer_public_key: Some(peer_key),
            contract: Some((contract, contract_key)),
            next_nonce: Some(nonce),
            activated: true,
            key_manager: self.km_client.clone(),
            io_ctx: IoContext::background().freeze(),
        }
    }

    /// Returns the raw underlying storage for the given `contract`--without
    /// encrypting the key or decrypting the return value.
    pub fn raw_storage(&mut self, contract: Address, storage_key: H256) -> Option<Vec<u8>> {
        self.execute_batch(|client, _ctx| {
            let state = client
                .cache
                .get_state(IoContext::background().freeze())
                .unwrap();
            state._storage_at(&contract, &storage_key).unwrap()
        })
    }

    /// Returns the key that actually stores the confidential contract's storage value.
    /// To be used together with `Client::raw_storage`.
    pub fn confidential_storage_key(&self, contract: Address, storage_key: H256) -> H256 {
        let km_confidential_ctx = self.key_manager_confidential_ctx(contract);
        keccak(
            &km_confidential_ctx
                .encrypt_storage(storage_key.to_vec())
                .unwrap(),
        )
    }

    /// Returns the storage expiry timestamp for a contract.
    pub fn storage_expiry(&mut self, contract: Address) -> u64 {
        self.execute_batch(|_client, ctx| methods::get_storage_expiry(&contract, ctx))
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
