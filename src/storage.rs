use ekiden_common::bytes::H256 as ekiden_H256;
use ekiden_core::futures::Future;
use ekiden_storage_base::{hash_storage_key, StorageBackend};
#[cfg(not(target_env = "sgx"))]
use ekiden_storage_dummy::DummyStorageBackend;
#[cfg(target_env = "sgx")]
use ekiden_trusted::db::untrusted::UntrustedStorageBackend;
use ethcore::storage::Storage;
use ethereum_types::H256;

use std::str::FromStr;
use std::sync::Arc;

pub struct StorageImpl {}

lazy_static! {
    static ref BACKEND: Arc<StorageBackend> = {
        #[cfg(not(target_env = "sgx"))]
        let backend = Arc::new(DummyStorageBackend::new());
        #[cfg(target_env = "sgx")]
        let backend = Arc::new(UntrustedStorageBackend::new());
        backend
    };
}

impl Storage for StorageImpl {
    fn request_bytes(&mut self, key: H256) -> Result<Vec<u8>, String> {
        let backend = BACKEND.clone();
        let result = backend
            .get(ekiden_H256::from_str(&format!("{:x}", key)).unwrap())
            .wait();
        match result {
            Ok(value) => Ok(value),
            Err(error) => Err(error.description().to_string()),
        }
    }

    fn store_bytes(&mut self, bytes: &[u8]) -> Result<H256, String> {
        let backend = BACKEND.clone();
        let result = backend.insert(bytes.to_vec(), <u64>::max_value()).wait();
        match result {
            Ok(_) => Ok(H256::from_slice(&hash_storage_key(bytes).0)),
            Err(error) => Err(error.description().to_string()),
        }
    }
}
