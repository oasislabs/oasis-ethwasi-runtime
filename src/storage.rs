use ekiden_db_trusted::{handle::DatabaseHandle, Database};
use ethcore::storage::Storage;

pub struct StorageImpl {}

impl Storage for StorageImpl {
    fn request_bytes(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        let mut db = DatabaseHandle::instance();
        db.get(key)
    }
}
