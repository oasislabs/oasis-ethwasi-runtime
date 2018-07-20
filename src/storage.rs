use ekiden_trusted::db::{handle::DatabaseHandle, Database};
use ethcore::storage::Storage;
use ethereum_types::H256;

pub struct StorageImpl {}

impl Storage for StorageImpl {
    fn request_bytes(&mut self, key: H256) -> Option<Vec<u8>> {
        let mut db = DatabaseHandle::instance();
        let mut key_bytes = Vec::new();
        key.copy_to(&mut key_bytes);
        db.get(&key_bytes)
    }

    fn store_bytes(&mut self, key: H256, bytes: &[u8]) {
        let mut db = DatabaseHandle::instance();
        let mut key_bytes = Vec::new();
        key.copy_to(&mut key_bytes);
        db.insert(&mut key_bytes, bytes);
    }
}
