use std::{collections::{hash_map::Entry, HashMap, HashSet},
          sync::{Arc, Mutex}};

use ekiden_core::{self, error::Result, futures::prelude::*};
use ekiden_storage_base::{hash_storage_key, InsertOptions, StorageBackend};
use ekiden_storage_lru::LruCacheStorageBackend;
use ekiden_trusted::db::{Database, DatabaseHandle};
use elastic_array::ElasticArray128;
use ethcore::{self,
              account_db::Factory as AccountFactory,
              block::{IsBlock, LockedBlock, OpenBlock},
              blockchain::{BlockChain, BlockProvider, ExtrasInsert},
              encoded::Block,
              engines::ForkChoice,
              factory::Factories,
              filter::Filter as EthcoreFilter,
              header::Header,
              kvdb::{self, KeyValueDB},
              rlp::NULL_RLP,
              state::backend::Wrapped as WrappedBackend,
              transaction::Action,
              types::{ids::BlockId,
                      log_entry::{LocalizedLogEntry, LogEntry},
                      receipt::TransactionOutcome,
                      BlockNumber}};
use ethereum_api::{BlockId as EkidenBlockId, Filter, Log, Receipt, Transaction};
use ethereum_types::{Address, H256, U256};
use hashdb::{DBValue, HashDB};
use keccak_hash::{keccak, KECCAK_NULL_RLP};

use super::evm::{get_contract_address, SPEC};

/// A backend for storing Ethereum state (e.g., a hash database).
type Backend = WrappedBackend;
/// Ethereum  state using the specified backend.
type State = ethcore::state::State<Backend>;

lazy_static! {
    static ref GLOBAL_CACHE: Mutex<Option<Cache>> = Mutex::new(None);
}

/// Create factories for various Ethereum data structures.
fn get_factories() -> Factories {
    Factories {
        // We must use the plain account factory as the non-plain one mangles keys
        // which prevents us from using storage directly.
        accountdb: AccountFactory::Plain,
        ..Default::default()
    }
}

/// Cache is the in-memory blockchain cache backed by the database.
pub struct Cache {
    /// Root hash where the last invocation finished at. If the root hash
    /// differs on next invocation, the cache will be cleared first.
    root_hash: ekiden_core::bytes::H256,
    /// Blockchain state database instance.
    blockchain_db: Arc<BlockchainStateDb>,
    /// Ethereum state backend.
    state_backend: Backend,
    /// Ethereum state database.
    state_db: StorageHashDB,
    /// Actual blockchain cache.
    chain: BlockChain,
}

impl Cache {
    /// Create a new in-memory cache for the given state root.
    pub fn new(
        storage: Arc<StorageBackend>,
        db: DatabaseHandle,
        root_hash: ekiden_core::bytes::H256,
    ) -> Self {
        let blockchain_db = Arc::new(BlockchainStateDb::new(db));
        let state_db = StorageHashDB::new(storage, blockchain_db.clone());
        // Initialize Ethereum state with the genesis block in case there is none.
        let state_backend =
            SPEC.ensure_db_good(WrappedBackend(Box::new(state_db.clone())), &get_factories())
                .expect("state to be initialized");

        Self {
            root_hash,
            blockchain_db: blockchain_db.clone(),
            state_backend,
            state_db,
            chain: Self::new_chain(blockchain_db),
        }
    }

    /// Fetches a global `Cache` instance for the given state root.
    ///
    /// In case the current global instance is not valid for the given state root,
    /// it will be replaced.
    pub fn from_global(
        storage: Arc<StorageBackend>,
        db: DatabaseHandle,
        root_hash: ekiden_core::bytes::H256,
    ) -> Cache {
        let mut maybe_cache = GLOBAL_CACHE.lock().unwrap();
        let mut cache = maybe_cache
            .take()
            .unwrap_or_else(|| Cache::new(storage, db, root_hash));

        if cache.root_hash != root_hash {
            // Root hash differs, re-create the block chain cache from scratch.
            cache.chain = Self::new_chain(cache.blockchain_db.clone());
            cache.root_hash = root_hash;
        }

        cache
    }

    /// Commit changes to global `Cache` instance.
    pub fn commit_global(mut self) -> ekiden_core::bytes::H256 {
        let mut maybe_cache = GLOBAL_CACHE.lock().unwrap();
        if maybe_cache.is_some() {
            panic!("Multiple concurrent cache commits");
        }

        // Commit any pending state updates.
        self.state_db.commit();
        // Commit any blockchain state updates.
        let root_hash = self.blockchain_db
            .commit()
            .expect("commit blockchain state");

        self.root_hash = root_hash;
        *maybe_cache = Some(self);

        root_hash
    }

    fn new_chain(blockchain_db: Arc<BlockchainStateDb>) -> BlockChain {
        BlockChain::new(
            Default::default(), /* config */
            &*SPEC.genesis_block(),
            blockchain_db,
        )
    }

    pub(crate) fn get_state(&self) -> Result<State> {
        let root = self.chain.best_block_header().state_root().clone();
        Ok(ethcore::state::State::from_existing(
            self.state_backend.clone(),
            root,
            U256::zero(), /* account_start_nonce */
            get_factories(),
        )?)
    }

    pub(crate) fn new_block(&self) -> Result<OpenBlock<'static>> {
        let parent = self.chain.best_block_header();
        Ok(OpenBlock::new(
            &*SPEC.engine,
            get_factories(),
            cfg!(debug_assertions),           /* tracing */
            self.state_backend.clone(),       /* state_db */
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

    pub fn add_block(&mut self, block: LockedBlock) -> Result<()> {
        let block = block.seal(&*SPEC.engine, Vec::new())?;

        // Queue the db operations necessary to insert this block.
        let mut db_tx = kvdb::DBTransaction::default();
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

        // Commit the insert to the in-memory blockchain cache.
        self.chain.commit();
        // Write blockchain updates.
        self.blockchain_db
            .write(db_tx)
            .expect("write blockchain updates");

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

/// Item pending insertion into respective backend.
enum PendingItem {
    /// Item that needs to be inserted into the storage backend.
    Storage(i32, Vec<u8>),
    /// Item that needs to be inserted into the blockchain state database
    /// due to the value not being content-addressable in our storage backend.
    State(i32, Vec<u8>),
}

/// Internal structures, shared by multiple `StorageHashDB` clones.
struct StorageHashDBInner {
    /// Storage backend.
    backend: Arc<StorageBackend>,
    /// Blockchain state database instance.
    blockchain_db: Arc<BlockchainStateDb>,
    /// Pending inserts.
    pending_inserts: HashMap<H256, PendingItem>,
}

/// Parity's `HashDB` backed by our `StorageBackend`.
#[derive(Clone)]
pub struct StorageHashDB {
    inner: Arc<Mutex<StorageHashDBInner>>,
}

impl StorageHashDB {
    /// Size of the in-memory storage cache (number of entries).
    const STORAGE_CACHE_SIZE: usize = 1024;
    // TODO: Handle storage expiry.
    const STORAGE_EXPIRY_TIME: u64 = u64::max_value() / 2;
    /// Column to use in the blockchain state database.
    const STATE_DB_COLUMN: Option<u32> = None;

    fn new(storage: Arc<StorageBackend>, blockchain_db: Arc<BlockchainStateDb>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(StorageHashDBInner {
                backend: Arc::new(LruCacheStorageBackend::new(
                    storage,
                    Self::STORAGE_CACHE_SIZE,
                )),
                blockchain_db,
                pending_inserts: HashMap::new(),
            })),
        }
    }

    fn hash_storage_key(value: &[u8]) -> H256 {
        H256::from(&hash_storage_key(value)[..])
    }

    /// Commit changes into the underlying store.
    fn commit(&self) {
        let mut inner = self.inner.lock().unwrap();
        let backend = inner.backend.clone();
        let blockchain_db = inner.blockchain_db.clone();

        for (key, item) in inner.pending_inserts.drain() {
            match item {
                PendingItem::Storage(ref_count, value) => {
                    if ref_count <= 0 {
                        // We cannot remove anything as the underlying store is immutable.
                        continue;
                    }

                    backend
                        .insert(value, Self::STORAGE_EXPIRY_TIME, InsertOptions::default())
                        .wait()
                        .expect("insert into storage");
                }
                PendingItem::State(ref_count, _) if ref_count <= 0 => {
                    // Remove from blockchain state database.
                    let mut tx = blockchain_db.transaction();
                    tx.delete(Self::STATE_DB_COLUMN, &key);
                    blockchain_db
                        .write(tx)
                        .expect("remove from blockchain state database");
                }
                PendingItem::State(_, value) => {
                    // Insert into blockchain state database.
                    let mut tx = blockchain_db.transaction();
                    tx.put_vec(Self::STATE_DB_COLUMN, &key, value);
                    blockchain_db
                        .write(tx)
                        .expect("insert into blockchain state database");
                }
            }
        }
    }
}

impl HashDB for StorageHashDB {
    fn keys(&self) -> HashMap<H256, i32> {
        unimplemented!();
    }

    fn get(&self, key: &H256) -> Option<DBValue> {
        if key == &KECCAK_NULL_RLP {
            return Some(DBValue::from_slice(&NULL_RLP));
        }

        let inner = self.inner.lock().unwrap();

        let result = match inner.pending_inserts.get(key) {
            Some(PendingItem::Storage(ref_count, value)) if ref_count >= &0 => {
                Some(ElasticArray128::from_slice(&value[..]))
            }
            Some(PendingItem::State(ref_count, value)) if ref_count > &0 => {
                Some(ElasticArray128::from_slice(&value[..]))
            }
            Some(PendingItem::Storage(ref_count, _)) if ref_count < &0 => None,
            Some(PendingItem::State(ref_count, _)) if ref_count < &0 => None,
            _ => {
                // First, try to fetch from storage backend.
                let storage_key = ekiden_core::bytes::H256::from(&key[..]);
                match inner.backend.get(storage_key).wait() {
                    Ok(result) => Some(ElasticArray128::from_vec(result)),
                    _ => {
                        // Then, try to fetch from blockchain state database.
                        inner
                            .blockchain_db
                            .get(Self::STATE_DB_COLUMN, &key[..])
                            .expect("fetch from blockchain db")
                    }
                }
            }
        };

        result
    }

    fn contains(&self, key: &H256) -> bool {
        self.get(key).is_some()
    }

    fn insert(&mut self, value: &[u8]) -> H256 {
        if value == &NULL_RLP {
            return KECCAK_NULL_RLP.clone();
        }

        let key = Self::hash_storage_key(value);

        let mut inner = self.inner.lock().unwrap();
        match inner.pending_inserts.entry(key) {
            Entry::Occupied(mut entry) => {
                let item = entry.get_mut();
                match item {
                    PendingItem::Storage(ref_count, value) => {
                        *ref_count += 1;
                        *value = value.to_vec();
                    }
                    _ => panic!("storage/state key conflict"),
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(PendingItem::Storage(1, value.to_vec()));
            }
        }

        key
    }

    fn emplace(&mut self, key: H256, value: DBValue) {
        if &*value == &NULL_RLP {
            return;
        }

        if key == Self::hash_storage_key(&value) {
            self.insert(&value);
        } else {
            // NOTE: This is currently used to store per-account code. The issue is that
            //       our storage uses SHA512/256 and key != H(value), so we cannot just
            //       insert into storage. We use the blockchain state database to store
            //       these values.

            let mut inner = self.inner.lock().unwrap();
            match inner.pending_inserts.entry(key) {
                Entry::Occupied(mut entry) => {
                    let item = entry.get_mut();
                    match item {
                        PendingItem::State(ref_count, value) => {
                            *ref_count += 1;
                            *value = value.to_vec();
                        }
                        _ => panic!("storage/state key conflict"),
                    }
                }
                Entry::Vacant(entry) => {
                    entry.insert(PendingItem::State(1, value.to_vec()));
                }
            }
        }
    }

    fn remove(&mut self, key: &H256) {
        if key == &KECCAK_NULL_RLP {
            return;
        }

        let mut inner = self.inner.lock().unwrap();
        match inner.pending_inserts.entry(key.clone()) {
            Entry::Occupied(mut entry) => {
                let item = entry.get_mut();
                match item {
                    PendingItem::Storage(ref_count, _) => {
                        *ref_count -= 1;
                    }
                    _ => panic!("tried to remove non-storage item"),
                }
            }
            Entry::Vacant(entry) => {
                // We assume state items are only used for storing contract code and
                // those are never removed, so we assume storage here.
                entry.insert(PendingItem::Storage(-1, vec![]));
            }
        }
    }
}

/// Blockchain state database.
pub struct BlockchainStateDb {
    db: Mutex<DatabaseHandle>,
}

impl BlockchainStateDb {
    /// Create new blockchain state database.
    fn new(db: DatabaseHandle) -> Self {
        Self { db: Mutex::new(db) }
    }

    /// Commits updates to the underlying database.
    fn commit(&self) -> Result<ekiden_core::bytes::H256> {
        let mut db = self.db.lock().unwrap();
        db.commit()
    }
}

// TODO: Move to util module.
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

// TODO: Move to util module.
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

// TODO: Move to util module.
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

impl kvdb::KeyValueDB for BlockchainStateDb {
    fn get(&self, col: Option<u32>, key: &[u8]) -> kvdb::Result<Option<kvdb::DBValue>> {
        let db = self.db.lock().unwrap();

        Ok(db.get(&get_key(col, key)).map(kvdb::DBValue::from_vec))
    }

    fn get_by_prefix(&self, _col: Option<u32>, _prefix: &[u8]) -> Option<Box<[u8]>> {
        unimplemented!();
    }

    fn write_buffered(&self, transaction: kvdb::DBTransaction) {
        let mut db = self.db.lock().unwrap();

        transaction.ops.iter().for_each(|op| match op {
            &kvdb::DBOp::Insert {
                ref key,
                ref value,
                col,
            } => {
                db.insert(&get_key(col, key), value.to_vec().as_slice());
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
    extern crate ekiden_storage_dummy;

    use self::ekiden_storage_dummy::DummyStorageBackend;
    use lazy_static;

    use super::*;

    #[test]
    fn test_create_chain() {
        let storage = Arc::new(DummyStorageBackend::new());

        Cache::new(
            storage.clone(),
            DatabaseHandle::new(storage),
            ekiden_core::bytes::H256::zero(),
        );
    }
}
