//! Common data structures shared by runtime and gateway.

extern crate ekiden_core;
extern crate ekiden_keymanager_client;
extern crate ekiden_keymanager_common;
extern crate ekiden_storage_base;
extern crate ekiden_storage_lru;
extern crate ekiden_trusted;
extern crate elastic_array;
extern crate ethcore;
extern crate ethereum_types;
extern crate hashdb;
extern crate keccak_hash;

#[cfg(feature = "test")]
#[macro_use]
extern crate lazy_static;

pub mod confidential;

use std::{
    collections::{hash_map::Entry, HashMap},
    sync::{Arc, Mutex},
};

use ekiden_core::{error::Result, futures::prelude::*};
use ekiden_storage_base::{hash_storage_key, InsertOptions, StorageBackend};
use ekiden_storage_lru::LruCacheStorageBackend;
use ekiden_trusted::db::Database;
use elastic_array::ElasticArray128;
use ethcore::{
    account_db::Factory as AccountFactory,
    factory::Factories,
    kvdb::{self, KeyValueDB},
    rlp::NULL_RLP,
    state::backend::Wrapped as WrappedBackend,
};
use ethereum_types::H256;
use hashdb::{DBValue, HashDB};
use keccak_hash::KECCAK_NULL_RLP;

/// A backend for storing Ethereum state (e.g., a hash database).
pub type Backend = WrappedBackend;
/// Ethereum state using the specified backend.
pub type State = ethcore::state::State<Backend>;

/// Gas parameters
pub const BLOCK_GAS_LIMIT: usize = 16_000_000;
pub const MIN_GAS_PRICE_GWEI: usize = 1;

/// Create factories for various Ethereum data structures.
pub fn get_factories() -> Factories {
    Factories {
        // We must use the plain account factory as the non-plain one mangles keys
        // which prevents us from using storage directly.
        accountdb: AccountFactory::Plain,
        ..Default::default()
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
struct StorageHashDBInner<T: Database + Send + Sync> {
    /// Storage backend.
    backend: Arc<StorageBackend>,
    /// Blockchain state database instance.
    blockchain_db: Arc<BlockchainStateDb<T>>,
    /// Pending inserts.
    pending_inserts: HashMap<H256, PendingItem>,
}

/// Parity's `HashDB` backed by our `StorageBackend`.
pub struct StorageHashDB<T: Database + Send + Sync> {
    inner: Arc<Mutex<StorageHashDBInner<T>>>,
}

impl<T> StorageHashDB<T>
where
    T: Database + Send + Sync,
{
    /// Size of the in-memory storage cache (number of entries).
    const STORAGE_CACHE_SIZE: usize = 1024;
    // TODO: Handle storage expiry.
    const STORAGE_EXPIRY_TIME: u64 = u64::max_value() / 2;
    /// Column to use in the blockchain state database.
    const STATE_DB_COLUMN: Option<u32> = None;

    pub fn new(storage: Arc<StorageBackend>, blockchain_db: Arc<BlockchainStateDb<T>>) -> Self {
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
    pub fn commit(&self) {
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

impl<T> Clone for StorageHashDB<T>
where
    T: Database + Send + Sync,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> HashDB for StorageHashDB<T>
where
    T: Database + Send + Sync,
{
    fn keys(&self) -> HashMap<H256, i32> {
        unimplemented!();
    }

    fn get(&self, key: &H256) -> Option<DBValue> {
        if key == &KECCAK_NULL_RLP {
            return Some(DBValue::from_slice(&NULL_RLP));
        }

        let inner = self.inner.lock().unwrap();

        let result = match inner.pending_inserts.get(key) {
            Some(PendingItem::Storage(ref_count, value)) if ref_count > &0 => {
                Some(ElasticArray128::from_slice(&value[..]))
            }
            Some(PendingItem::State(ref_count, value)) if ref_count > &0 => {
                Some(ElasticArray128::from_slice(&value[..]))
            }
            Some(PendingItem::Storage(ref_count, _)) if ref_count <= &0 => {
                // Reference count indicates missing item, but the item may be
                // available in storage. It would be tempting to just return it
                // from cache but we need to make sure that the item exists in
                // external storage as doing otherwise could lead to state
                // corruption since pending items with zero or negative reference
                // count are not persisted.
                let storage_key = ekiden_core::bytes::H256::from(&key[..]);
                match inner.backend.get(storage_key).wait() {
                    Ok(result) => Some(ElasticArray128::from_vec(result)),
                    _ => None,
                }
            }
            Some(PendingItem::State(ref_count, _)) if ref_count <= &0 => {
                // Reference count indicates missing item, but the item may be
                // available in storage. It would be tempting to just return it
                // from cache but we need to make sure that the item exists in
                // external storage as doing otherwise could lead to state
                // corruption since pending items with zero or negative reference
                // count are not persisted.
                inner
                    .blockchain_db
                    .get(Self::STATE_DB_COLUMN, &key[..])
                    .expect("fetch from blockchain db must succeed")
            }
            _ => {
                // Key is not in local cache. First try to fetch it from the
                // storage backend.
                let storage_key = ekiden_core::bytes::H256::from(&key[..]);
                match inner.backend.get(storage_key).wait() {
                    Ok(result) => Some(ElasticArray128::from_vec(result)),
                    _ => {
                        // Then, try to fetch from blockchain state database.
                        inner
                            .blockchain_db
                            .get(Self::STATE_DB_COLUMN, &key[..])
                            .expect("fetch from blockchain db must succeed")
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
                    PendingItem::Storage(ref_count, stored_value) => {
                        *ref_count += 1;
                        *stored_value = value.to_vec();
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

        // NOTE: This is currently used to store per-account code. The issue is that
        //       our storage uses SHA512/256 and key != H(value), so we cannot just
        //       insert into storage. We use the blockchain state database to store
        //       these values.

        let mut inner = self.inner.lock().unwrap();
        match inner.pending_inserts.entry(key) {
            Entry::Occupied(mut entry) => {
                let item = entry.get_mut();
                match item {
                    PendingItem::State(ref_count, stored_value) => {
                        *ref_count += 1;
                        *stored_value = value.to_vec();
                    }
                    _ => panic!("storage/state key conflict"),
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(PendingItem::State(1, value.to_vec()));
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

#[derive(Debug)]
/// Blockchain state database.
pub struct BlockchainStateDb<T: Database + Send + Sync> {
    db: Mutex<T>,
}

impl<T> BlockchainStateDb<T>
where
    T: Database + Send + Sync,
{
    /// Create new blockchain state database.
    pub fn new(db: T) -> Self {
        Self { db: Mutex::new(db) }
    }

    /// Return current state root hash.
    pub fn get_root_hash(&self) -> ekiden_core::bytes::H256 {
        self.db.lock().unwrap().get_root_hash()
    }

    /// Commits updates to the underlying database.
    pub fn commit(&self) -> Result<ekiden_core::bytes::H256> {
        let mut db = self.db.lock().unwrap();
        db.commit()
    }
}

// Parity expects the database to namespace keys by column. The Ekiden db
// doesn't [yet?] have this feature, so we emulate by prepending the column id
// to the actual key. Columns None and 0 should be distinct, so we use prefix 0
// for None and col+1 for Some(col).
pub fn get_key(col: Option<u32>, key: &[u8]) -> Vec<u8> {
    let col_bytes = col.map(|id| (id + 1).to_le_bytes()).unwrap_or([0, 0, 0, 0]);
    col_bytes
        .into_iter()
        .chain(key.into_iter())
        .map(|v| v.to_owned())
        .collect()
}

impl<T> kvdb::KeyValueDB for BlockchainStateDb<T>
where
    T: Database + Send + Sync,
{
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

    use ethereum_types::H256;
    use hashdb::{DBValue, HashDB};

    use self::ekiden_storage_dummy::DummyStorageBackend;
    use ekiden_storage_base::hash_storage_key;
    use ekiden_trusted::db::DatabaseHandle;

    use super::*;

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
    fn test_remove_before_insert() {
        let storage = Arc::new(DummyStorageBackend::new());
        let db = DatabaseHandle::new(storage.clone());
        let blockchain_db = Arc::new(BlockchainStateDb::new(db));
        let mut hash_db = StorageHashDB::new(storage.clone(), blockchain_db);

        let hw_key = H256::from(&hash_storage_key(b"hello world")[..]);
        // Remove key from db, reference count should be -1, value empty.
        hash_db.remove(&hw_key);
        assert_eq!(hash_db.get(&hw_key), None);
        // Insert key to db, reference count should be 0, value overwritten.
        hash_db.insert(b"hello world");
        assert_eq!(hash_db.get(&hw_key), None);
        // Insert key to db, reference count should be 1, value overwritten.
        hash_db.insert(b"hello world");
        assert_eq!(
            hash_db.get(&hw_key),
            Some(DBValue::from_slice(b"hello world"))
        );
    }

    #[test]
    fn test_storage_hashdb() {
        let storage = Arc::new(DummyStorageBackend::new());
        let db = DatabaseHandle::new(storage.clone());
        let blockchain_db = Arc::new(BlockchainStateDb::new(db));
        let mut hash_db = StorageHashDB::new(storage.clone(), blockchain_db);

        assert_eq!(hash_db.get(&H256::zero()), None);
        let hw_key = hash_db.insert(b"hello world");
        assert_eq!(
            hash_db.get(&hw_key),
            Some(DBValue::from_slice(b"hello world"))
        );
        hash_db.insert(b"hello world");
        hash_db.insert(b"hello world");
        assert_eq!(
            hash_db.get(&hw_key),
            Some(DBValue::from_slice(b"hello world"))
        );
        hash_db.remove(&hw_key);
        assert_eq!(
            hash_db.get(&hw_key),
            Some(DBValue::from_slice(b"hello world"))
        );
        hash_db.remove(&hw_key);
        assert_eq!(
            hash_db.get(&hw_key),
            Some(DBValue::from_slice(b"hello world"))
        );
        hash_db.remove(&hw_key);
        assert_eq!(hash_db.get(&hw_key), None);

        hash_db.remove(&hw_key);
        hash_db.insert(b"hello world");
        assert_eq!(hash_db.get(&hw_key), None);

        hash_db.remove(&hw_key);
        hash_db.remove(&hw_key);
        hash_db.insert(b"hello world");
        assert_eq!(hash_db.get(&hw_key), None);
        hash_db.insert(b"hello world");
        assert_eq!(hash_db.get(&hw_key), None);
        hash_db.insert(b"hello world");
        assert_eq!(
            hash_db.get(&hw_key),
            Some(DBValue::from_slice(b"hello world"))
        );

        // Commit and re-create database.
        hash_db.commit();
        let db = DatabaseHandle::new(storage.clone());
        let blockchain_db = Arc::new(BlockchainStateDb::new(db));
        let hash_db = StorageHashDB::new(storage.clone(), blockchain_db);
        assert_eq!(
            hash_db.get(&hw_key),
            Some(DBValue::from_slice(b"hello world"))
        );
    }
}
