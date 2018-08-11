//! Read-only interface to blockchain and account state, backed by an Ekiden Database.

use std::marker::{Send, Sync};
use std::sync::Arc;

use common_types::log_entry::{LocalizedLogEntry, LogEntry};
use ethcore;
use ethcore::blockchain::{BlockDetails, BlockProvider, BlockReceipts, TransactionAddress};
use ethcore::db::{self, Readable};
use ethcore::encoded;
use ethcore::header::BlockNumber;
use ethcore::state::backend::Basic as BasicBackend;
use ethereum_types::{Bloom, H256, U256};
use journaldb::overlaydb::OverlayDB;
use kvdb::{self, KeyValueDB};
use rayon::prelude::*;
use rlp_compress::{blocks_swapper, decompress};

use ekiden_db_trusted::Database;

type Backend = BasicBackend<OverlayDB>;
pub type EthState = ethcore::state::State<Backend>;

pub struct StateDb<T: Database + Send + Sync> {
    db: Arc<T>,
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
        match self.get(db::COL_HEADERS, &hash) {
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
        match self.get(db::COL_BODIES, hash) {
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
        self.read(db::COL_EXTRA, hash)
    }

    fn block_hash(&self, index: BlockNumber) -> Option<H256> {
        self.read(db::COL_EXTRA, &index)
    }

    fn transaction_address(&self, hash: &H256) -> Option<TransactionAddress> {
        self.read(db::COL_EXTRA, hash)
    }

    fn block_receipts(&self, hash: &H256) -> Option<BlockReceipts> {
        self.read(db::COL_EXTRA, hash)
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
    pub fn new(db: T) -> Option<Self> {
        let state_db = Self { db: Arc::new(db) };
        match state_db.best_block_hash() {
            Some(_) => Some(state_db),
            None => None,
        }
    }

    // returns None if the database has not been initialized (i.e., no best block state root)
    pub fn get_ethstate(&self) -> Option<EthState> {
        let root = self.best_block_state_root()?;
        let backend = BasicBackend(OverlayDB::new(
            Arc::new(StateDb {
                db: self.db.clone(),
            }),
            None, /* col */
        ));
        match ethcore::state::State::from_existing(
            backend,
            root,
            U256::zero(),       /* account_start_nonce */
            Default::default(), /* factories */
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
        match self.get(db::COL_EXTRA, b"best") {
            Ok(best) => best.map(|best| H256::from_slice(&best)),
            Err(e) => {
                measure_counter_inc!("read_state_failed");
                error!("Could not get best block hash from database: {:?}", e);
                None
            }
        }
    }

    fn best_block_state_root(&self) -> Option<H256> {
        match self.best_block_hash() {
            Some(hash) => self.block_header_data(&hash)
                .map(|h| h.state_root().clone()),
            None => None,
        }
    }

    pub fn best_block_number(&self) -> BlockNumber {
        match self.best_block_hash() {
            Some(hash) => self.block_header_data(&hash)
                .map(|h| h.number())
                .unwrap_or(0),
            None => 0,
        }
    }
}

// Parity expects the database to namespace keys by column. The Ekiden db
// doesn't [yet?] have this feature, so we emulate by prepending the column id
// to the actual key. Columns None and 0 should be distinct, so we use prefix 0
// for None and col+1 for Some(col).
pub fn get_key(col: Option<u32>, key: &[u8]) -> Vec<u8> {
    let col_bytes = col.map(|id| (id + 1).to_le().to_bytes())
        .unwrap_or([0, 0, 0, 0]);
    col_bytes
        .into_iter()
        .chain(key.into_iter())
        .map(|v| v.to_owned())
        .collect()
}

impl<T> kvdb::KeyValueDB for StateDb<T>
where
    T: Database + Send + Sync,
{
    // we only use get
    fn get(&self, col: Option<u32>, key: &[u8]) -> kvdb::Result<Option<kvdb::DBValue>> {
        Ok(self.db.get(&get_key(col, key)).map(kvdb::DBValue::from_vec))
    }

    fn get_by_prefix(&self, _col: Option<u32>, _prefix: &[u8]) -> Option<Box<[u8]>> {
        unimplemented!();
    }

    // this is a read only interface
    fn write_buffered(&self, _transaction: kvdb::DBTransaction) {
        unimplemented!();
    }

    fn flush(&self) -> kvdb::Result<()> {
        unimplemented!();
    }

    fn iter<'a>(&'a self, _col: Option<u32>) -> Box<Iterator<Item = (Box<[u8]>, Box<[u8]>)> + 'a> {
        unimplemented!();
    }

    fn iter_from_prefix<'a>(
        &'a self,
        _col: Option<u32>,
        _prefix: &'a [u8],
    ) -> Box<Iterator<Item = (Box<[u8]>, Box<[u8]>)> + 'a> {
        unimplemented!();
    }

    fn restore(&self, _new_db: &str) -> kvdb::Result<()> {
        unimplemented!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethereum_types::{Address, H256, U256};
    use test_helpers::MockDb;

    #[test]
    fn test_get_key() {
        let value = b"somevalue";
        let col_none = get_key(None, value);
        let col_0 = get_key(Some(0), value);
        assert_ne!(col_none, col_0);

        // prefix for column Some(3) is 4=3+1
        let col_3 = get_key(Some(3), b"three");
        assert_eq!(col_3, b"\x04\0\0\0three");
    }

    #[test]
    fn test_get_statedb_empty() {
        let state = StateDb::new(MockDb::new());
        assert!(state.is_none());
    }

    #[test]
    fn test_get_statedb() {
        let mut db = MockDb::new();
        // insert a valid best block hash
        db.insert(
            &get_key(db::COL_EXTRA, b"best"),
            &H256::from("0xec891bd71e6d6a64ec299b8641c6cce3638989c03a4a41fd5898a2c0356c7ae6"),
        );
        let state = StateDb::new(db);
        assert!(state.is_some());
    }

    #[test]
    fn test_best_block() {
        let mut db = MockDb::new();
        // populate the db with test data
        db.populate();
        let state = StateDb::new(db).unwrap();
        assert_eq!(state.best_block_number(), 4);
    }

    #[test]
    fn test_logs() {
        use ethcore::client::BlockId;
        use ethcore::filter::Filter;

        let mut db = MockDb::new();
        // populate the db with test data
        db.populate();

        // get state
        let state = StateDb::new(db).unwrap();

        // all blocks
        let blocks = vec![
            H256::from("f39c325375fa2d5381a950850abd9999abd2ff64cd0f184139f5bb5d74afb14e"),
            H256::from("d56eee931740bb35eb9bf9f97cfebb66ac51a1d88988c1255b52677b958d658b"),
            H256::from("17a7a94ad21879641349b6e90ccd7e42e63551ad81b3fda561cd2df4860fbd3f"),
            H256::from("c57db28f3a012eb2a783cd1295a0c5e7fcc08565c526c2c86c8355a54ab7aae3"),
            H256::from("339ddee2b78be3e53af2b0a3148643973cf0e0fa98e16ab963ee17bf79e6f199"),
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

        // one log entry expected
        assert_eq!(logs.len(), 1);
    }

    #[test]
    fn test_account_state() {
        let mut db = MockDb::new();
        // populate the db with test data
        db.populate();

        // get state
        let state = StateDb::new(db).unwrap();

        // get ethstate
        let ethstate = state.get_ethstate().unwrap();

        // an account in the genesis block containing 100 ETH, no storage, and no code
        let balance_only = Address::from("7110316b618d20d0c44728ac2a3d683536ea682b");
        let balance = ethstate.balance(&balance_only).unwrap();
        assert_eq!(balance, U256::from("56bc75e2d63100000"));
        let code = ethstate.code(&balance_only).unwrap().unwrap();
        assert_eq!(code.len(), 0);
        let val = ethstate.storage_at(&balance_only, &H256::zero()).unwrap();
        assert_eq!(val, H256::zero());
        let nonce = ethstate.nonce(&balance_only).unwrap();
        assert_eq!(nonce, U256::zero());

        // a deployed contract
        let deployed_contract = Address::from("345ca3e014aaf5dca488057592ee47305d9b3e10");
        let code = ethstate.code(&deployed_contract).unwrap().unwrap();
        assert!(code.len() > 0);
    }

    #[test]
    fn test_transaction() {
        let mut db = MockDb::new();
        // populate the db with test data
        db.populate();

        // get state
        let state = StateDb::new(db).unwrap();

        // get the transaction from block 4
        let tx = state
            .transaction_address(&H256::from(
                "0xcfb3d83aa4b9c7d9a698e9b8169383c819fbf6200848ae5fcaec25e414295790",
            ))
            .and_then(|addr| BlockProvider::transaction(&state, &addr))
            .unwrap();

        assert_eq!(tx.block_number, 4);
    }

    #[test]
    fn test_receipt() {
        let mut db = MockDb::new();
        // populate the db with test data
        db.populate();

        // get state
        let state = StateDb::new(db).unwrap();

        let receipt = state
            .transaction_address(&H256::from(
                "0xcfb3d83aa4b9c7d9a698e9b8169383c819fbf6200848ae5fcaec25e414295790",
            ))
            .and_then(|addr| state.transaction_receipt(&addr))
            .unwrap();

        assert_eq!(receipt.logs.len(), 1);
    }

    #[test]
    fn test_block() {
        let mut db = MockDb::new();
        // populate the db with test data
        db.populate();

        // get state
        let state = StateDb::new(db).unwrap();

        // get best block
        let best_block = state
            .best_block_hash()
            .and_then(|hash| state.block(&hash))
            .unwrap();

        assert_eq!(best_block.header_view().number(), 4);
    }
}
