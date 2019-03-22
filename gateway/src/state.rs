//! Read-only interface to blockchain and account state, backed by an Ekiden Database.

use std::{
    marker::{Send, Sync},
    sync::Arc,
};

use common_types::log_entry::{LocalizedLogEntry, LogEntry};
use ethcore::{
    self,
    blockchain::{BlockDetails, BlockProvider, BlockReceipts, TransactionAddress},
    db::{self, Readable},
    encoded,
    header::BlockNumber,
    ids::BlockId,
    state::backend::Wrapped as WrappedBackend,
};
use ethereum_types::{Bloom, H256, U256};
use kvdb::KeyValueDB;
use rayon::prelude::*;
use rlp_compress::{blocks_swapper, decompress};

use ekiden_db_trusted::Database;
use ekiden_storage_base::StorageBackend;
pub use runtime_ethereum_common::{confidential::ConfidentialCtx, State as EthState};
use runtime_ethereum_common::{get_factories, Backend, BlockchainStateDb, StorageHashDB};

pub struct StateDb<T: Database + Send + Sync> {
    /// Blockchain state database instance.
    blockchain_db: Arc<BlockchainStateDb<T>>,
    /// Ethereum state backend.
    state_backend: Backend,
}

impl<T> BlockProvider for StateDb<T>
where
    T: Database + Send + Sync,
{
    fn block(&self, hash: &H256) -> Option<encoded::Block> {
        let header = self.block_header_data(hash)?;
        let body = self.block_body(hash)?;
        Some(encoded::Block::new_from_header_and_body(
            &header.view(),
            &body.view(),
        ))
    }

    fn block_header_data(&self, hash: &H256) -> Option<encoded::Header> {
        match self.blockchain_db.get(db::COL_HEADERS, &hash) {
            Ok(hash) => {
                hash.map(|h| encoded::Header::new(decompress(&h, blocks_swapper()).into_vec()))
            }
            Err(e) => {
                measure_counter_inc!("read_state_failed");
                error!("Could not get block header from database: {:?}", e);
                None
            }
        }
    }

    fn block_body(&self, hash: &H256) -> Option<encoded::Body> {
        match self.blockchain_db.get(db::COL_BODIES, hash) {
            Ok(body) => {
                body.map(|b| encoded::Body::new(decompress(&b, blocks_swapper()).into_vec()))
            }
            Err(e) => {
                measure_counter_inc!("read_state_failed");
                error!("Could not get block body from database: {:?}", e);
                None
            }
        }
    }

    fn block_details(&self, hash: &H256) -> Option<BlockDetails> {
        self.blockchain_db.read(db::COL_EXTRA, hash)
    }

    fn block_hash(&self, index: BlockNumber) -> Option<H256> {
        self.blockchain_db.read(db::COL_EXTRA, &index)
    }

    fn transaction_address(&self, hash: &H256) -> Option<TransactionAddress> {
        self.blockchain_db.read(db::COL_EXTRA, hash)
    }

    fn block_receipts(&self, hash: &H256) -> Option<BlockReceipts> {
        self.blockchain_db.read(db::COL_EXTRA, hash)
    }

    /// Returns logs matching given filter. The order of logs returned will be the same as the order of the blocks
    /// provided. And it's the callers responsibility to sort blocks provided in advance.
    fn logs<F>(
        &self,
        mut blocks: Vec<H256>,
        matches: F,
        limit: Option<usize>,
    ) -> Vec<LocalizedLogEntry>
    where
        F: Fn(&LogEntry) -> bool + Send + Sync,
        Self: Sized,
    {
        // sort in reverse order
        blocks.reverse();

        let mut logs = blocks
            .chunks(128)
            .flat_map(move |blocks_chunk| {
                blocks_chunk
                    .into_par_iter()
                    .filter_map(|hash| self.block_number(&hash).map(|r| (r, hash)))
                    .filter_map(|(number, hash)| {
                        self.block_receipts(&hash)
                            .map(|r| (number, hash, r.receipts))
                    })
                    .filter_map(|(number, hash, receipts)| {
                        self.block_body(&hash)
                            .map(|ref b| (number, hash, receipts, b.transaction_hashes()))
                    })
                    .flat_map(|(number, hash, mut receipts, mut hashes)| {
                        if receipts.len() != hashes.len() {
                            warn!("Block {} ({}) has different number of receipts ({}) to transactions ({}). Database corrupt?", number, hash, receipts.len(), hashes.len());
                            assert!(false);
                        }
                        let mut log_index = receipts
                            .iter()
                            .fold(0, |sum, receipt| sum + receipt.logs.len());

                        let receipts_len = receipts.len();
                        hashes.reverse();
                        receipts.reverse();
                        receipts
                            .into_iter()
                            .map(|receipt| receipt.logs)
                            .zip(hashes)
                            .enumerate()
                            .flat_map(move |(index, (mut logs, tx_hash))| {
                                let current_log_index = log_index;
                                let no_of_logs = logs.len();
                                log_index -= no_of_logs;

                                logs.reverse();
                                logs.into_iter().enumerate().map(move |(i, log)| {
                                    LocalizedLogEntry {
                                        entry: log,
                                        block_hash: *hash,
                                        block_number: number,
                                        transaction_hash: tx_hash,
                                        // iterating in reverse order
                                        transaction_index: receipts_len - index - 1,
                                        transaction_log_index: no_of_logs - i - 1,
                                        log_index: current_log_index - i - 1,
                                    }
                                })
                            })
                            .filter(|log_entry| matches(&log_entry.entry))
                            .take(limit.unwrap_or(::std::usize::MAX))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            })
            .take(limit.unwrap_or(::std::usize::MAX))
            .collect::<Vec<LocalizedLogEntry>>();
        logs.reverse();
        logs
    }

    // we don't use the remaining functions
    fn is_known(&self, _hash: &H256) -> bool {
        unimplemented!();
    }

    fn first_block(&self) -> Option<H256> {
        unimplemented!();
    }

    fn best_ancient_block(&self) -> Option<H256> {
        unimplemented!();
    }

    fn best_ancient_number(&self) -> Option<BlockNumber> {
        unimplemented!();
    }

    fn blocks_with_bloom(
        &self,
        _bloom: &Bloom,
        _from_block: BlockNumber,
        _to_block: BlockNumber,
    ) -> Vec<BlockNumber> {
        unimplemented!();
    }
}

impl<T> StateDb<T>
where
    T: 'static + Database + Send + Sync,
{
    // returns None if the database has not been initialized (i.e., no best block)
    pub fn new(storage: Arc<StorageBackend>, db: T) -> Result<Option<Self>, String> {
        let blockchain_db = Arc::new(BlockchainStateDb::new(db));
        let state_db = StorageHashDB::new(storage, blockchain_db.clone());
        let state_backend = WrappedBackend(Box::new(state_db.clone()));

        match blockchain_db.get(db::COL_EXTRA, b"best") {
            Ok(best) => Ok(best.map(|_| Self {
                blockchain_db,
                state_backend,
            })),
            Err(e) => Err(e.to_string()),
        }
    }

    // returns None if the database has not been initialized
    pub fn get_ethstate_at(&self, id: BlockId) -> Option<EthState> {
        let root = self.state_root_at(id)?;
        match ethcore::state::State::from_existing(
            self.state_backend.clone(),
            root,
            U256::zero(), /* account_start_nonce */
            get_factories(),
            Some(Box::new(ConfidentialCtx::new())),
        ) {
            Ok(state) => Some(state),
            Err(e) => {
                measure_counter_inc!("read_state_failed");
                error!("Could not get EthState from database: {:?}", e);
                None
            }
        }
    }

    pub fn best_block_hash(&self) -> Option<H256> {
        match self.blockchain_db.get(db::COL_EXTRA, b"best") {
            Ok(best) => best.map(|best| H256::from_slice(&best)),
            Err(e) => {
                measure_counter_inc!("read_state_failed");
                error!("Could not get best block hash from database: {:?}", e);
                None
            }
        }
    }

    fn state_root_at(&self, block: BlockId) -> Option<H256> {
        let hash = match block {
            BlockId::Hash(hash) => Some(hash),
            BlockId::Number(number) => self.block_hash(number),
            BlockId::Earliest => self.block_hash(0),
            BlockId::Latest => self.best_block_hash(),
        };
        match hash {
            Some(hash) => self
                .block_header_data(&hash)
                .map(|h| h.state_root().clone()),
            None => None,
        }
    }

    pub fn best_block_number(&self) -> BlockNumber {
        match self.best_block_hash() {
            Some(hash) => self
                .block_header_data(&hash)
                .map(|h| h.number())
                .unwrap_or(0),
            None => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethereum_types::{Address, H256, U256};
    use runtime_ethereum_common::get_key;
    use test_helpers::MockDb;

    #[test]
    fn test_get_statedb_empty() {
        let db = MockDb::empty();
        let state = StateDb::new(db.storage(), db).unwrap();
        assert!(state.is_none());
    }

    #[test]
    fn test_get_statedb() {
        let mut db = MockDb::empty();
        // insert a valid best block hash
        db.insert(
            &get_key(db::COL_EXTRA, b"best"),
            &H256::from("0xec891bd71e6d6a64ec299b8641c6cce3638989c03a4a41fd5898a2c0356c7ae6"),
        );
        let state = StateDb::new(db.storage(), db).unwrap();
        assert!(state.is_some());
    }

    #[test]
    fn test_best_block() {
        let db = MockDb::new();
        let state = StateDb::new(db.storage(), db).unwrap().unwrap();
        assert_eq!(state.best_block_number(), 10);
    }

    #[test]
    fn test_logs() {
        use ethcore::{filter::Filter, ids::BlockId};

        let db = MockDb::new();

        // get state
        let state = StateDb::new(db.storage(), db).unwrap().unwrap();

        // all blocks
        let blocks = vec![
            H256::from("9b26126b79590cf25dea37b63a513aefd7e5d775124478ee2118988bffec6dd8"),
            H256::from("155032cef8c377f1aa81a0b968852ea552c3944f2b2addc89197b53e5f0ed618"),
            H256::from("b71fb084bd34f31dca06a3f47b3c182ba7f8848dd25e45f36413bf6047a96b07"),
            H256::from("32185fcbe326513f77f85135dc5a913b1e5a645076e5ed2e34bc6ec7bc3268d4"),
            H256::from("779d5d6d648b5dc2136d8aefa50dc99c960626f6c52ed4b06045835de8b5c70d"),
            H256::from("e34d21062a4b605fda1e2d4b832fef615bd90d2704d1bbe209b1c2cb8e03905d"),
            H256::from("b1a04a31b23c3ad0dccf0c757a94463cfca1265966bc66efaf08a427e668e088"),
            H256::from("bac57123063dd9cf9a9406996a6ec6d3f5ab93cd16a05318365784477f30f8a5"),
            H256::from("834deb56b3560fff98cbbb72dc0ea1e890cc8c32d675c80d52cab70ffbbd817f"),
            H256::from("bacdbc2ed8161be77ed20a490e71f080017a39a1e81975e3a732da3e3d1b416b"),
            H256::from("c6c2b9de0cd02f617035534d69ac1413f184e5f5adf41bef9ae6271f18308778"),
        ];

        // query over all blocks
        let filter = Filter {
            from_block: BlockId::Earliest,
            to_block: BlockId::Latest,
            address: None,
            topics: vec![None, None, None, None],
            limit: None,
        };

        // get logs
        let logs = state.logs(blocks, |entry| filter.matches(entry), filter.limit);

        // four logs expected
        assert_eq!(logs.len(), 4);
    }

    #[test]
    fn test_account_state() {
        let db = MockDb::new();

        // get state
        let state = StateDb::new(db.storage(), db).unwrap().unwrap();

        // get ethstate at latest block
        let ethstate = state.get_ethstate_at(BlockId::Latest).unwrap();

        // an account in the genesis block containing 100000000000 ETH, no storage, and no code
        let balance_only = Address::from("7110316b618d20d0c44728ac2a3d683536ea682b");
        let balance = ethstate.balance(&balance_only).unwrap();
        assert_eq!(balance, U256::from("1431e0fae6d7217caa0000000"));
        let code = ethstate.code(&balance_only).unwrap().unwrap();
        assert_eq!(code.len(), 0);
        let val = ethstate.storage_at(&balance_only, &H256::zero()).unwrap();
        assert_eq!(val, H256::zero());
        let nonce = ethstate.nonce(&balance_only).unwrap();
        assert_eq!(nonce, U256::zero());

        // a deployed contract
        let deployed_contract = Address::from("fbe2ab6ee22dace9e2ca1cb42c57bf94a32ddd41");
        let code = ethstate.code(&deployed_contract).unwrap().unwrap();
        assert!(code.len() > 0);
    }

    #[test]
    fn test_transaction() {
        let db = MockDb::new();

        // get state
        let state = StateDb::new(db.storage(), db).unwrap().unwrap();

        // get the transaction from block 10
        let tx = state
            .transaction_address(&H256::from(
                "0x584be3ae3b766f4ca4353ab3ee8e54bab4bda00aa264165c5792a868496c568f",
            ))
            .and_then(|addr| BlockProvider::transaction(&state, &addr))
            .unwrap();

        assert_eq!(tx.block_number, 10);
    }

    #[test]
    fn test_receipt() {
        let db = MockDb::new();

        // get state
        let state = StateDb::new(db.storage(), db).unwrap().unwrap();

        // get the transaction from block 10
        let receipt = state
            .transaction_address(&H256::from(
                "0x584be3ae3b766f4ca4353ab3ee8e54bab4bda00aa264165c5792a868496c568f",
            ))
            .and_then(|addr| state.transaction_receipt(&addr))
            .unwrap();

        assert_eq!(receipt.logs.len(), 1);
    }

    #[test]
    fn test_block() {
        let db = MockDb::new();

        // get state
        let state = StateDb::new(db.storage(), db).unwrap().unwrap();

        // get best block
        let best_block = state
            .best_block_hash()
            .and_then(|hash| state.block(&hash))
            .unwrap();

        assert_eq!(best_block.header_view().number(), 10);
    }

    #[test]
    fn test_default_block_parameter() {
        let db = MockDb::new();

        // get state
        let state = StateDb::new(db.storage(), db).unwrap().unwrap();

        // a deployed contract
        let deployed_contract = Address::from("fbe2ab6ee22dace9e2ca1cb42c57bf94a32ddd41");

        // get ethstate at block 0
        let ethstate_0 = state.get_ethstate_at(BlockId::Number(0)).unwrap();
        // code should be empty at block 0
        let code_0 = ethstate_0.code(&deployed_contract).unwrap();
        assert!(code_0.is_none());

        // get ethstate at latest block
        let ethstate_latest = state.get_ethstate_at(BlockId::Latest).unwrap();
        // code should be non-empty at latest block
        let code_latest = ethstate_latest.code(&deployed_contract).unwrap().unwrap();
        assert!(code_latest.len() > 0);
    }
}
