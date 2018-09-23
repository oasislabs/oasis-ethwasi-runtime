use std::{collections::HashSet,
          ops::Deref,
          sync::{Arc, Mutex}};

use ekiden_core::{self, error::Result};
use ekiden_trusted::db::{Database, DatabaseHandle};
use ethcore::{self,
              block::{Drain, IsBlock, LockedBlock, OpenBlock},
              blockchain::{BlockChain, BlockProvider, ExtrasInsert},
              encoded::Block,
              engines::ForkChoice,
              filter::Filter as EthcoreFilter,
              header::Header,
              journaldb::overlaydb::OverlayDB,
              kvdb::{self, KeyValueDB},
              state::backend::Basic as BasicBackend,
              transaction::Action,
              types::{ids::BlockId,
                      log_entry::{LocalizedLogEntry, LogEntry},
                      receipt::TransactionOutcome,
                      BlockNumber}};
use ethereum_api::{BlockId as EkidenBlockId, Filter, Log, Receipt, Transaction};
use ethereum_types::{Address, H256, U256};

use super::evm::{get_contract_address, SPEC};

/// Cache is the in-memory blockchain cache backed by the database.
pub struct Cache {
    /// Root hash where the last invocation finished at. If the root hash
    /// differs on next invocation, the cache will be cleared first.
    root_hash: Option<ekiden_core::bytes::H256>,
    /// State database instance.
    state_db: Arc<StateDb>,
    /// Actual blockchain cache.
    chain: BlockChain,
}

impl Cache {
    fn new() -> Self {
        let mut db = SPEC.ensure_db_good(get_backend(), &Default::default() /* factories */)
            .unwrap();
        db.0.commit().unwrap();

        let state_db = Arc::new(StateDb::instance());

        Self {
            root_hash: None,
            state_db: state_db.clone(),
            chain: Self::new_chain(state_db),
        }
    }

    /// Invokes a closure with a Cache instance valid for the current state root.
    pub fn for_current_state_root<F, R>(f: F) -> R
    where
        F: FnOnce(&Cache) -> R,
    {
        lazy_static! {
            static ref GLOBAL_CACHE: Mutex<Cache> = Mutex::new(Cache::new());
        }

        let mut cache = GLOBAL_CACHE.lock().unwrap();
        let root_hash = Some(DatabaseHandle::instance().get_root_hash().unwrap());
        if cache.root_hash != root_hash {
            // Root hash differs, re-create the block chain cache from scratch.
            cache.chain = Self::new_chain(cache.state_db.clone());
            cache.root_hash = root_hash;
        }

        let result = f(cache.deref());

        // Update the root hash.
        cache.root_hash = Some(DatabaseHandle::instance().get_root_hash().unwrap());

        result
    }

    fn new_chain(state_db: Arc<StateDb>) -> BlockChain {
        BlockChain::new(
            Default::default(), /* config */
            &*SPEC.genesis_block(),
            state_db,
        )
    }

    pub(crate) fn get_backend(&self) -> Backend {
        BasicBackend(OverlayDB::new(self.state_db.clone(), None /* col */))
    }

    pub(crate) fn get_state(&self) -> Result<State> {
        let backend = self.get_backend();
        let root = self.chain.best_block_header().state_root().clone();
        Ok(ethcore::state::State::from_existing(
            backend,
            root,
            U256::zero(),       /* account_start_nonce */
            Default::default(), /* factories */
        )?)
    }

    pub(crate) fn new_block(&self) -> Result<OpenBlock<'static>> {
        let parent = self.chain.best_block_header();
        Ok(OpenBlock::new(
            &*SPEC.engine,
            Default::default(),               /* factories */
            cfg!(debug_assertions),           /* tracing */
            self.get_backend(),               /* state_db */
            &parent,                          /* parent */
            self.last_hashes(&parent.hash()), /* last hashes */
            Address::default(),               /* author */
            (U256::one(), U256::max_value()), /* gas_range_target */
            vec![],                           /* extra data */
            true,                             /* is epoch_begin */
            &mut Vec::new().into_iter(),      /* ancestry */
        )?)
    }

    pub fn get_account_storage(&self, address: Address, key: H256) -> Result<H256> {
        Ok(self.get_state()?.storage_at(&address, &key)?)
    }

    pub fn get_account_nonce(&self, address: &Address) -> Result<U256> {
        Ok(self.get_state()?.nonce(&address)?)
    }

    pub fn get_account_balance(&self, address: &Address) -> Result<U256> {
        Ok(self.get_state()?.balance(&address)?)
    }

    pub fn get_account_code(&self, address: &Address) -> Result<Option<Vec<u8>>> {
        // convert from Option<Arc<Vec<u8>>> to Option<Vec<u8>>
        Ok(self.get_state()?.code(&address)?.map(|c| (&*c).clone()))
    }

    fn block_number_ref(&self, id: &BlockId) -> Option<BlockNumber> {
        match *id {
            BlockId::Number(number) => Some(number),
            BlockId::Hash(ref hash) => self.chain.block_number(hash),
            BlockId::Earliest => Some(0),
            BlockId::Latest => Some(self.chain.best_block_number()),
        }
    }

    pub fn get_logs(&self, filter: &Filter) -> Vec<Log> {
        let filter = EthcoreFilter {
            from_block: to_block_id(filter.from_block.clone()),
            to_block: to_block_id(filter.to_block.clone()),
            address: match filter.address.clone() {
                Some(address) => Some(address.into_iter().map(Into::into).collect()),
                None => None,
            },
            topics: filter.topics.clone().into_iter().map(Into::into).collect(),
            limit: filter.limit.map(Into::into),
        };

        // if either the from or to block is invalid, return an empty Vec
        let from = match self.block_number_ref(&filter.from_block) {
            Some(n) => n,
            None => return vec![],
        };
        let to = match self.block_number_ref(&filter.to_block) {
            Some(n) => n,
            None => return vec![],
        };

        let blocks = filter.bloom_possibilities().iter()
            .map(|bloom| {
                self.chain.blocks_with_bloom(bloom, from, to)
            })
        .flat_map(|m| m)
            // remove duplicate elements
            .collect::<HashSet<u64>>()
            .into_iter()
            .filter_map(|n| self.chain.block_hash(n))
            .collect::<Vec<H256>>();

        self.chain
            .logs(blocks, |entry| filter.matches(entry), filter.limit)
            .into_iter()
            .map(lle_to_log)
            .collect()
    }

    pub fn last_hashes(&self, parent_hash: &H256) -> Arc<Vec<H256>> {
        let mut last_hashes = vec![];
        last_hashes.resize(256, H256::default());
        last_hashes[0] = parent_hash.clone();
        for i in 0..255 {
            match self.chain.block_details(&last_hashes[i]) {
                Some(details) => {
                    last_hashes[i + 1] = details.parent.clone();
                }
                None => break,
            }
        }
        Arc::new(last_hashes)
    }

    pub fn add_block(&self, block: LockedBlock) -> Result<()> {
        let block = block.seal(&*SPEC.engine, Vec::new())?;

        let mut db_tx = kvdb::DBTransaction::default();

        // queue the db ops necessary to insert this block
        self.chain.insert_block(
            &mut db_tx,
            &block.rlp_bytes(),
            block.receipts().to_owned(),
            ExtrasInsert {
                fork_choice: ForkChoice::New,
                is_finalized: true,
                metadata: None,
            },
        );

        self.chain.commit(); // commit the insert to the in-memory BlockChain repr
        let mut db = block.drain().0;
        db.commit_to_batch(&mut db_tx)
            .expect("could not commit state updates"); // add any pending state updates to the db transaction
        StateDb::instance()
            .write(db_tx)
            .expect("could not persist state updates"); // persist the changes to the backing db

        Ok(())
    }

    pub fn get_transaction(&self, hash: &H256) -> Option<Transaction> {
        let addr = self.chain.transaction_address(hash)?;
        let mut tx = self.chain.transaction(&addr)?;
        let signature = tx.signature();
        Some(Transaction {
            hash: tx.hash(),
            nonce: tx.nonce,
            block_hash: Some(tx.block_hash),
            block_number: Some(U256::from(tx.block_number)),
            index: Some(tx.transaction_index.into()),
            from: tx.sender(),
            to: match tx.action {
                Action::Create => None,
                Action::Call(address) => Some(address),
            },
            value: tx.value,
            gas_price: tx.gas_price,
            gas: tx.gas,
            input: tx.data.clone(),
            creates: match tx.action {
                Action::Create => Some(get_contract_address(&tx.sender(), &tx)),
                Action::Call(_) => None,
            },
            raw: ::rlp::encode(&tx.signed).into_vec(),
            // TODO: recover pubkey
            public_key: None,
            chain_id: tx.chain_id().into(),
            standard_v: tx.standard_v().into(),
            v: tx.original_v().into(),
            r: signature.r().into(),
            s: signature.s().into(),
        })
    }

    pub fn get_receipt(&self, hash: &H256) -> Option<Receipt> {
        let addr = self.chain.transaction_address(hash)?;
        let mut tx = self.chain.transaction(&addr)?;
        let receipt = self.chain.transaction_receipt(&addr)?;
        Some(Receipt {
            hash: Some(tx.hash()),
            index: Some(U256::from(addr.index)),
            block_hash: Some(tx.block_hash),
            block_number: Some(U256::from(tx.block_number)),
            cumulative_gas_used: receipt.gas_used, // TODO: get from block header
            gas_used: Some(receipt.gas_used),
            contract_address: match tx.action {
                Action::Create => Some(get_contract_address(&tx.sender(), &tx)),
                Action::Call(_) => None,
            },
            logs: receipt.logs.into_iter().map(le_to_log).collect(),
            logs_bloom: receipt.log_bloom,
            state_root: match receipt.outcome {
                TransactionOutcome::StateRoot(hash) => Some(hash),
                _ => None,
            },
            status_code: match receipt.outcome {
                TransactionOutcome::StatusCode(code) => Some(code.into()),
                _ => None,
            },
        })
    }

    pub fn block_hash(&self, number: BlockNumber) -> Option<H256> {
        self.chain.block_hash(number)
    }

    pub fn block_by_number(&self, number: BlockNumber) -> Option<Block> {
        self.chain
            .block_hash(number)
            .and_then(|hash| self.chain.block(&hash))
    }

    pub fn block_by_hash(&self, hash: H256) -> Option<Block> {
        self.chain.block(&hash)
    }

    pub fn get_latest_block_number(&self) -> BlockNumber {
        self.chain.best_block_number()
    }

    pub fn best_block_header(&self) -> Header {
        self.chain.best_block_header()
    }
}

pub struct StateDb {}

type Backend = BasicBackend<OverlayDB>;
type State = ethcore::state::State<Backend>;

pub(crate) fn get_backend() -> Backend {
    BasicBackend(OverlayDB::new(
        Arc::new(StateDb::instance()),
        None, /* col */
    ))
}

impl StateDb {
    fn new() -> Self {
        Self {}
    }

    pub fn instance() -> Self {
        Self::new()
    }
}

fn lle_to_log(lle: LocalizedLogEntry) -> Log {
    Log {
        address: lle.entry.address,
        topics: lle.entry.topics.into_iter().map(Into::into).collect(),
        data: lle.entry.data.into(),
        block_hash: Some(lle.block_hash),
        block_number: Some(lle.block_number.into()),
        transaction_hash: Some(lle.transaction_hash),
        transaction_index: Some(lle.transaction_index.into()),
        log_index: Some(lle.log_index.into()),
        transaction_log_index: Some(lle.transaction_log_index.into()),
    }
}

fn le_to_log(le: LogEntry) -> Log {
    Log {
        address: le.address,
        topics: le.topics.into_iter().map(Into::into).collect(),
        data: le.data.into(),
        block_hash: None,
        block_number: None,
        transaction_hash: None,
        transaction_index: None,
        log_index: None,
        transaction_log_index: None,
    }
}

fn to_block_id(id: EkidenBlockId) -> BlockId {
    match id {
        EkidenBlockId::Number(number) => BlockId::Number(number.into()),
        EkidenBlockId::Hash(hash) => BlockId::Hash(hash),
        EkidenBlockId::Earliest => BlockId::Earliest,
        EkidenBlockId::Latest => BlockId::Latest,
    }
}

// Parity expects the database to namespace keys by column. The Ekiden db
// doesn't [yet?] have this feature, so we emulate by prepending the column id
// to the actual key. Columns None and 0 should be distinct, so we use prefix 0
// for None and col+1 for Some(col).
fn get_key(col: Option<u32>, key: &[u8]) -> Vec<u8> {
    let col_bytes = col.map(|id| (id + 1).to_le_bytes()).unwrap_or([0, 0, 0, 0]);
    col_bytes
        .into_iter()
        .chain(key.into_iter())
        .map(|v| v.to_owned())
        .collect()
}

impl kvdb::KeyValueDB for StateDb {
    fn get(&self, col: Option<u32>, key: &[u8]) -> kvdb::Result<Option<kvdb::DBValue>> {
        Ok(DatabaseHandle::instance()
            .get(&get_key(col, key))
            .map(kvdb::DBValue::from_vec))
    }

    fn get_by_prefix(&self, _col: Option<u32>, _prefix: &[u8]) -> Option<Box<[u8]>> {
        unimplemented!();
    }

    fn write_buffered(&self, transaction: kvdb::DBTransaction) {
        transaction.ops.iter().for_each(|op| match op {
            &kvdb::DBOp::Insert {
                ref key,
                ref value,
                col,
            } => {
                DatabaseHandle::instance().insert(&get_key(col, key), value.to_vec().as_slice());
            }
            &kvdb::DBOp::Delete { .. } => {
                // This is a no-op for us. Parity cleans up old state (anything
                // not part of the trie defined by the best block state root).
                // We want to retain previous states to support web3 APIs that
                // take a default block parameter:
                // https://github.com/ethereum/wiki/wiki/JSON-RPC#the-default-block-parameter
            }
        });
    }

    fn flush(&self) -> kvdb::Result<()> {
        Ok(())
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
    use lazy_static;

    #[test]
    fn test_create_chain() {
        Cache::for_current_state_root(|_| {});
    }
}
