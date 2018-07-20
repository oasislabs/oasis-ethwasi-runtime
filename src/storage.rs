use ekiden_common::bytes::H256 as EkidenH256;
use ekiden_core::futures::Future;
use ekiden_storage_base::{hash_storage_key, StorageBackend};
#[cfg(not(target_env = "sgx"))]
use ekiden_storage_dummy::DummyStorageBackend as StorageBackendImpl;
#[cfg(target_env = "sgx")]
use ekiden_trusted::db::untrusted::UntrustedStorageBackend as StorageBackendImpl;
use ethcore::{storage::Storage,
              vm::{Error, Result}};
use ethereum_types::H256;

use std::str::FromStr;
use std::sync::Arc;

pub struct StorageImpl {}

impl StorageImpl {
    pub fn new() -> Self {
        StorageImpl {}
    }
}

lazy_static! {
    static ref BACKEND: Arc<StorageBackend> = Arc::new(StorageBackendImpl::new());
}

impl Storage for StorageImpl {
    fn request_bytes(&mut self, key: H256) -> Result<Vec<u8>> {
        let result = BACKEND
            .get(EkidenH256::from_str(&format!("{:x}", key)).unwrap())
            .wait();
        result.map_err(|err| Error::Storage(err.description().to_string()))
    }

    fn store_bytes(&mut self, bytes: &[u8]) -> Result<H256> {
        let result = BACKEND.insert(bytes.to_vec(), <u64>::max_value()).wait();
        match result {
            Ok(_) => Ok(H256::from_slice(&hash_storage_key(bytes).0)),
            Err(err) => Err(Error::Storage(err.description().to_string())),
        }
    }

    fn store_bytes(&mut self, key: H256, bytes: &[u8]) {
        let mut db = DatabaseHandle::instance();
        let mut key_bytes = Vec::new();
        key.copy_to(&mut key_bytes);
        db.insert(&mut key_bytes, bytes);
    }
}
