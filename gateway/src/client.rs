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
use journaldb::overlaydb::OverlayDB;
use transaction::{LocalizedTransaction, SignedTransaction};

type Backend = BasicBackend<OverlayDB>;

pub struct Client {}

impl Client {
    pub fn new() -> Self {
        Self {}
    }

    pub fn instance() -> Self {
        Self::new()
    }

    // block-related
    pub fn best_block_number(&self) -> BlockNumber {
        unimplemented!()
    }

    pub fn block(&self, id: BlockId) -> Option<encoded::Block> {
        /*
        let chain = self.chain.read();
        Self::block_hash(&chain, id).and_then(|hash| chain.block(&hash))
        */
        unimplemented!()
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
        unimplemented!()
    }

    pub fn block_header(&self, id: BlockId) -> Option<encoded::Header> {
        /*
        let chain = self.chain.read();
        Self::block_hash(&chain, id).and_then(|hash| chain.block_header_data(&hash))
        */
        unimplemented!()
    }

    pub fn block_status(&self, id: BlockId) -> BlockStatus {
        /*
        let chain = self.chain.read();
        match Self::block_hash(&chain, id) {
            Some(ref hash) if chain.is_known(hash) => BlockStatus::InChain,
            None => BlockStatus::Unknown
        }
        */
        unimplemented!()
    }

    // transaction-related
    pub fn transaction(&self, id: TransactionId) -> Option<LocalizedTransaction> {
        unimplemented!()
    }

    pub fn transaction_receipt(&self, id: TransactionId) -> Option<LocalizedReceipt> {
        unimplemented!()
    }

    pub fn logs(&self, filter: Filter) -> Vec<LocalizedLogEntry> {
        unimplemented!()
    }

    // account state-related
    pub fn balance(&self, address: &Address, state: StateOrBlock) -> Option<U256> {
        unimplemented!()
    }

    pub fn code(&self, address: &Address, state: StateOrBlock) -> Option<Option<Bytes>> {
        unimplemented!()
    }

    pub fn nonce(&self, address: &Address, id: BlockId) -> Option<U256> {
        unimplemented!()
    }

    pub fn storage_at(
        &self,
        address: &Address,
        position: &H256,
        state: StateOrBlock,
    ) -> Option<H256> {
        unimplemented!()
    }

    // state-related
    pub fn state_at(&self, id: BlockId) -> Option<State<Backend>> {
        unimplemented!()
    }

    // evm-related
    pub fn call(
        &self,
        transaction: &SignedTransaction,
        analytics: CallAnalytics,
        state: &mut State<Backend>,
        header: &Header,
    ) -> Result<Executed, CallError> {
        unimplemented!()
    }

    pub fn estimate_gas(
        &self,
        transaction: &SignedTransaction,
        state: &State<Backend>,
        header: &Header,
    ) -> Result<U256, CallError> {
        unimplemented!()
    }

    pub fn send_raw_transaction(&self, raw: Bytes) -> Result<H256, CallError> {
        // TODO: call runtime-evm contract
        unimplemented!()
    }
}
