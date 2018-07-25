use std::{mem, sync::Arc};

use common_types::log_entry::{LocalizedLogEntry, LogEntry};
use ethcore;
use ethcore::blockchain::{BlockReceipts, TransactionAddress};
use ethcore::db::{self, Readable};
use ethcore::encoded;
use ethcore::filter::Filter;
use ethcore::header::BlockNumber;
use ethcore::receipt::{LocalizedReceipt, Receipt};
use ethcore::state::backend::Basic as BasicBackend;
use ethereum_types::{H256, U256};
use journaldb::overlaydb::OverlayDB;
use kvdb::{self, KeyValueDB};
use rayon::prelude::*;
use rlp_compress::{blocks_swapper, decompress};
use transaction::LocalizedTransaction;

use client_utils::db::Snapshot;
use ekiden_db_trusted::Database;

pub struct StateDb {
    snapshot: Snapshot,
}

impl StateDb {
    pub fn new(snapshot: Snapshot) -> Option<Self> {
        let db = Self { snapshot: snapshot };
        match db.best_block_hash() {
            Some(_) => Some(db),
            None => None,
        }
    }

    pub fn best_block_hash(&self) -> Option<H256> {
        match self.get(db::COL_EXTRA, b"best") {
            Ok(best) => best.map(|best| H256::from_slice(&best)),
            Err(e) => {
                warn!("Could not fetch best_block_hash from snapshot: {:?}", e);
                None
            }
        }
    }

    pub fn block_header_data(&self, hash: &H256) -> Option<encoded::Header> {
        match self.get(db::COL_HEADERS, &hash) {
            Ok(hash) => {
                hash.map(|h| encoded::Header::new(decompress(&h, blocks_swapper()).into_vec()))
            }
            Err(e) => {
                warn!("Could not fetch block_header_data from snapshot: {:?}", e);
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
                warn!("Could not fetch block_body from snapshot: {:?}", e);
                None
            }
        }
    }

    pub fn block(&self, hash: &H256) -> Option<encoded::Block> {
        let header = self.block_header_data(hash)?;
        let body = self.block_body(hash)?;
        Some(encoded::Block::new_from_header_and_body(
            &header.view(),
            &body.view(),
        ))
    }

    pub fn block_hash(&self, index: BlockNumber) -> Option<H256> {
        self.read(db::COL_EXTRA, &index)
    }

    pub fn block_number(&self, hash: &H256) -> Option<BlockNumber> {
        self.block_header_data(hash).map(|header| header.number())
    }

    // convenience function
    pub fn best_block_state_root(&self) -> Option<H256> {
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

    pub fn transaction(&self, address: &TransactionAddress) -> Option<LocalizedTransaction> {
        self.block_body(&address.block_hash).and_then(|body| {
            self.block_number(&address.block_hash).and_then(|n| {
                body.view()
                    .localized_transaction_at(&address.block_hash, n, address.index)
            })
        })
    }

    pub fn transaction_address(&self, hash: &H256) -> Option<TransactionAddress> {
        self.read(db::COL_EXTRA, hash)
    }

    fn block_receipts(&self, hash: &H256) -> Option<BlockReceipts> {
        self.read(db::COL_EXTRA, hash)
    }

    pub fn transaction_receipt(&self, hash: &H256) -> Option<Receipt> {
        let address: TransactionAddress = self.transaction_address(hash)?;
        self.block_receipts(&address.block_hash)
            .and_then(|br| br.receipts.into_iter().nth(address.index))
    }

    /// Returns logs matching given filter. The order of logs returned will be the same as the order of the blocks
    /// provided. And it's the callers responsibility to sort blocks provided in advance.
    pub fn logs<F>(
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
}

type Backend = BasicBackend<OverlayDB>;
pub type EthState = ethcore::state::State<Backend>;

pub fn get_ethstate(snapshot: Snapshot) -> Option<EthState> {
    if let Some(db) = StateDb::new(snapshot) {
        let root = db.best_block_state_root()?;
        let backend = BasicBackend(OverlayDB::new(Arc::new(db), None /* col */));
        match ethcore::state::State::from_existing(
            backend,
            root,
            U256::zero(),       /* account_start_nonce */
            Default::default(), /* factories */
        ) {
            Ok(state) => Some(state),
            Err(e) => {
                error!("Could not construct EthState from snapshot");
                None
            }
        }
    } else {
        None
    }
}

pub fn to_bytes(num: u32) -> [u8; mem::size_of::<u32>()] {
    unsafe { mem::transmute(num) }
}

// parity expects the database to namespace keys by column
// the ekiden db doesn't [yet?] have this feature, so we emulate by
// prepending the column id to the actual key
fn get_key(col: Option<u32>, key: &[u8]) -> Vec<u8> {
    let col_bytes = col.map(|id| to_bytes(id.to_le())).unwrap_or([0, 0, 0, 0]);
    col_bytes
        .into_iter()
        .chain(key.into_iter())
        .map(|v| v.to_owned())
        .collect()
}

impl kvdb::KeyValueDB for StateDb {
    fn get(&self, col: Option<u32>, key: &[u8]) -> kvdb::Result<Option<kvdb::DBValue>> {
        Ok(self.snapshot
            .get(&get_key(col, key))
            .map(kvdb::DBValue::from_vec))
    }

    fn get_by_prefix(&self, _col: Option<u32>, _prefix: &[u8]) -> Option<Box<[u8]>> {
        unimplemented!();
    }

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
