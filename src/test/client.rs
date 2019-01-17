//! Test client to interact with a runtime-ethereum blockchain.

use ekiden_core::mrae::nonce::{Nonce, NONCE_SIZE};
use ekiden_keymanager_common::ContractKey;
use ethcore::{rlp,
              state::ConfidentialCtx as EthConfidentialCtx,
              transaction::{Action, Transaction as EthcoreTransaction}};
use ethereum_api::TransactionRequest;
use ethereum_types::{Address, H256, U256};
use ethkey::{KeyPair, Secret};
use runtime_ethereum_common::confidential::{key_manager::TestKeyManager, ConfidentialCtx,
                                            CONFIDENTIAL_PREFIX};
use std::str::FromStr;
use std::sync::{Mutex, MutexGuard};
use test::*;

lazy_static! {
    static ref CLIENT: Mutex<Client> = Mutex::new(Client::new());
}

pub struct Client {
    /// KeyPair used for signing transactions.
    pub keypair: KeyPair,
    /// Contract key used for encrypting web3c transactions.
    pub ephemeral_key: ContractKey,
}

impl Client {
    fn new() -> Self {
        Self {
            // address: 0x7110316b618d20d0c44728ac2a3d683536ea682
            keypair: KeyPair::from_secret(
                Secret::from_str(
                    "533d62aea9bbcb821dfdda14966bb01bfbbb53b7e9f5f0d69b8326e052e3450c",
                ).unwrap(),
            ).unwrap(),
            ephemeral_key: TestKeyManager::create_random_key(),
        }
    }

    /// Returns a handle to the client to interact with the blockchain.
    pub fn instance<'a>() -> MutexGuard<'a, Self> {
        CLIENT.lock().unwrap()
    }

    /// Creates a non-confidential contract, return the transaction hash for the deploy
    /// and the address of the contract.
    pub fn create_contract(&mut self, code: Vec<u8>, balance: &U256) -> (H256, Address) {
        let hash = self.send(None, code, balance);
        let receipt = with_batch_handler(|ctx| get_receipt(&hash, ctx).unwrap().unwrap());
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

        with_batch_handler(|ctx| simulate_transaction(&tx, ctx).unwrap().result.unwrap())
    }

    /// Sends a transaction onchain that updates the blockchain, analagous to the web3.js send().
    pub fn send(&mut self, contract: Option<&Address>, data: Vec<u8>, value: &U256) -> H256 {
        with_batch_handler(|ctx| {
            let tx = EthcoreTransaction {
                action: if contract == None {
                    Action::Create
                } else {
                    Action::Call(*contract.unwrap())
                },
                nonce: get_account_nonce(&self.keypair.address(), ctx).unwrap(),
                gas_price: U256::from(0),
                gas: U256::from(1000000),
                value: *value,
                data: data,
            }.sign(&self.keypair.secret(), None);

            let raw = rlp::encode(&tx);
            execute_raw_transaction(&raw.into_vec(), ctx)
                .unwrap()
                .hash
                .unwrap()
        })
    }
}
