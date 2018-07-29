use std::{mem, sync::Arc};

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

use client_utils::db::Snapshot;
use ekiden_db_trusted::Database;

type Backend = BasicBackend<OverlayDB>;
pub type EthState = ethcore::state::State<Backend>;

pub struct StateDb {
    snapshot: Arc<Snapshot>,
}

impl BlockProvider for StateDb {
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
                error!("Could not get block header from snapshot: {:?}", e);
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
                error!("Could not get block body from snapshot: {:?}", e);
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

impl StateDb {
    // returns None if the database has not been initialized (i.e., no best block)
    pub fn new(snapshot: Snapshot) -> Option<Self> {
        let db = Self {
            snapshot: Arc::new(snapshot),
        };
        match db.best_block_hash() {
            Some(_) => Some(db),
            None => None,
        }
    }

    // returns None if the database has not been initialized (i.e., no best block state root)
    pub fn get_ethstate(&self) -> Option<EthState> {
        let root = self.best_block_state_root()?;
        let backend = BasicBackend(OverlayDB::new(
            Arc::new(StateDb {
                snapshot: self.snapshot.clone(),
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
                error!("Could not get EthState from snapshot: {:?}", e);
                None
            }
        }
    }

    pub fn best_block_hash(&self) -> Option<H256> {
        match self.get(db::COL_EXTRA, b"best") {
            Ok(best) => best.map(|best| H256::from_slice(&best)),
            Err(e) => {
                error!("Could not get best block hash from snapshot: {:?}", e);
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

fn to_bytes(num: u32) -> [u8; mem::size_of::<u32>()] {
    unsafe { mem::transmute(num) }
}

// Parity expects the database to namespace keys by column. The Ekiden db
// doesn't [yet?] have this feature, so we emulate by prepending the column id
// to the actual key. Columns None and 0 should be distinct, so we use the
// prefix 0xffffffff for None.
fn get_key(col: Option<u32>, key: &[u8]) -> Vec<u8> {
    let col_bytes = col.map(|id| to_bytes(id.to_le()))
        .unwrap_or([0xff, 0xff, 0xff, 0xff]);
    col_bytes
        .into_iter()
        .chain(key.into_iter())
        .map(|v| v.to_owned())
        .collect()
}

impl kvdb::KeyValueDB for StateDb {
    // we only use get
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

#[cfg(test)]
mod tests {
    #[test]
    fn test_get_key() {
        use super::get_key;

        let value = b"somevalue";
        let col_none = get_key(None, value);
        let col_0 = get_key(Some(0), value);
        assert_ne!(col_none, col_0);

        let col_3 = get_key(Some(3), b"three");
        assert_eq!(col_3, b"\x03\0\0\0three");
    }
}
