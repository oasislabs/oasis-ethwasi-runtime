use ekiden_common::bytes::H256 as EkidenH256;
use ekiden_common::futures::FutureExt;
use ekiden_core::futures::Future;
use ekiden_storage_base::{hash_storage_key, InsertOptions, StorageBackend};
#[cfg(not(target_env = "sgx"))]
use ekiden_storage_dummy::DummyStorageBackend as StorageBackendImpl;
#[cfg(target_env = "sgx")]
use ekiden_trusted::db::untrusted::UntrustedStorageBackend as StorageBackendImpl;
use ethcore::vm::{Error, Result};
use ethereum_types::H256;

use std::str::FromStr;
use std::sync::Arc;

lazy_static! {
    static ref BACKEND: Arc<StorageBackend> = Arc::new(StorageBackendImpl::new());
}

pub struct GlobalStorage;

impl GlobalStorage {
    pub fn new() -> Self {
        GlobalStorage {}
    }
}

impl GlobalStorage {
    fn fetch_bytes(&self, key: &H256) -> Result<Vec<u8>> {
        Ok(vec![])
    }

    fn store_bytes(&self, bytes: &[u8]) -> Result<H256> {
        Ok(H256::from(0))
    }
}

/// Return the storage backend used by global storage.
#[cfg(not(target_env = "sgx"))]
pub fn get_storage_backend() -> Arc<StorageBackend> {
    BACKEND.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage() {
        let storage = GlobalStorage::new();
        let val = "wonderwall".as_bytes();
        let key = storage.store_bytes(&val).unwrap();
        let result = storage.fetch_bytes(&key).unwrap();
        assert_eq!(val, result.as_slice());
    }
}
