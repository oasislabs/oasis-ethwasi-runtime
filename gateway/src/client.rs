use bytes::Bytes;
use ethcore::client::{BlockId, BlockStatus, CallAnalytics, StateOrBlock, TransactionId};
use ethcore::encoded;
use ethcore::error::CallError;
use ethcore::executive::Executed;
use ethcore::filter::Filter;
use ethcore::header::{BlockNumber, Header};
use ethcore::log_entry::LocalizedLogEntry;
use ethcore::receipt::LocalizedReceipt;
use ethcore::state::backend::Basic as BasicBackend;
use ethcore::state::State;
use ethereum_types::{Address, H256, U256};
use futures::future::Future;
use journaldb::overlaydb::OverlayDB;
use runtime_evm;
use rustc_hex::FromHex;
use transaction::{LocalizedTransaction, SignedTransaction, Transaction};

use evm_api::TransactionRequest;

type Backend = BasicBackend<OverlayDB>;

pub struct Client {
    client: runtime_evm::Client,
}

impl Client {
    pub fn new(client: runtime_evm::Client) -> Self {
        Self { client: client }
    }

    // block-related
    pub fn best_block_number(&self) -> BlockNumber {
        let block_height = self.client.get_block_height(false).wait().unwrap();
        block_height.into()
    }

    pub fn block(&self, id: BlockId) -> Option<encoded::Block> {
        /*
        let chain = self.chain.read();
        Self::block_hash(&chain, id).and_then(|hash| chain.block(&hash))
        */
        let response = if let BlockId::Hash(hash) = id {
            self.client.get_block_by_hash(hash).wait().unwrap()
        } else {
            let number = match id {
                BlockId::Hash(hash) => unreachable!(),
                BlockId::Number(number) => format!("{:x}", number),
                BlockId::Earliest => "0".to_owned(),
                BlockId::Latest => "latest".to_owned(),
            };
            self.client.get_block_by_number(number).wait().unwrap()
        };

        match response {
            Some(block) => Some(encoded::Block::new(block)),
            None => None,
        }
    }

    pub fn block_hash(&self, id: BlockId) -> Option<H256> {
        /*
        match id {
            BlockId::Hash(hash) => Some(hash),
            BlockId::Number(number) => chain.block_hash(number),
            BlockId::Earliest => chain.block_hash(0),
            BlockId::Latest => Some(chain.best_block_hash()),
        }
        */
        None
    }

    pub fn block_header(&self, id: BlockId) -> Option<encoded::Header> {
        /*
        let chain = self.chain.read();
        Self::block_hash(&chain, id).and_then(|hash| chain.block_header_data(&hash))
        */
        None
    }

    pub fn block_status(&self, id: BlockId) -> BlockStatus {
        /*
        let chain = self.chain.read();
        match Self::block_hash(&chain, id) {
            Some(ref hash) if chain.is_known(hash) => BlockStatus::InChain,
            None => BlockStatus::Unknown
        }
        */
        BlockStatus::Unknown
    }

    // transaction-related
    pub fn transaction(&self, id: TransactionId) -> Option<LocalizedTransaction> {
        None
    }

    pub fn transaction_receipt(&self, id: TransactionId) -> Option<LocalizedReceipt> {
        None
    }

    pub fn logs(&self, filter: Filter) -> Vec<LocalizedLogEntry> {
        vec![]
    }

    // account state-related
    pub fn balance(&self, address: &Address, state: StateOrBlock) -> Option<U256> {
        let balance = self.client.get_account_balance(*address).wait().unwrap();
        Some(balance)
    }

    pub fn code(&self, address: &Address, state: StateOrBlock) -> Option<Option<Bytes>> {
        // TODO: differentiate between no account vs no code
        let code = self.client.get_account_code(*address).wait().unwrap();
        match FromHex::from_hex(code.as_str()) {
            Ok(bytes) => Some(Some(bytes)),
            Err(_) => Some(None),
        }
    }

    pub fn nonce(&self, address: &Address, id: BlockId) -> Option<U256> {
        let nonce = self.client.get_account_nonce(*address).wait().unwrap();
        Some(nonce)
    }

    pub fn storage_at(
        &self,
        address: &Address,
        position: &H256,
        state: StateOrBlock,
    ) -> Option<H256> {
        let value = self.client
            .get_storage_at((*address, *position))
            .wait()
            .unwrap();
        Some(value)
    }

    // state-related
    pub fn state_at(&self, id: BlockId) -> Option<State<Backend>> {
        None
    }

    // evm-related
    pub fn call(&self, request: TransactionRequest) -> Result<Bytes, CallError> {
        match self.client.simulate_transaction(request).wait() {
            Ok(result) => Ok(result.result),
            Err(e) => Err(CallError::Exceptional),
        }
    }

    pub fn estimate_gas(&self, request: TransactionRequest) -> Result<U256, CallError> {
        match self.client.simulate_transaction(request).wait() {
            Ok(result) => Ok(result.used_gas),
            Err(e) => Err(CallError::Exceptional),
        }
    }

    pub fn send_raw_transaction(&self, raw: Bytes) -> Result<H256, CallError> {
        match self.client.execute_raw_transaction(raw).wait() {
            Ok(result) => Ok(result),
            Err(e) => Err(CallError::Exceptional),
        }
    }

    pub fn send_transaction(&self, request: TransactionRequest) -> Result<H256, CallError> {
        match self.client
            .debug_execute_unsigned_transaction(request)
            .wait()
        {
            Ok(result) => Ok(result),
            Err(e) => Err(CallError::Exceptional),
        }
    }
}
