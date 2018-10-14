use ekiden_common::bytes::H256 as EkidenH256;
use ekiden_common::futures::FutureExt;
use ekiden_core::futures::Future;
use ekiden_storage_base::{hash_storage_key, InsertOptions, StorageBackend};
#[cfg(not(target_env = "sgx"))]
use ekiden_storage_dummy::DummyStorageBackend as StorageBackendImpl;
#[cfg(target_env = "sgx")]
use ekiden_trusted::db::untrusted::UntrustedStorageBackend as StorageBackendImpl;
use ethcore::{storage::Storage,
              vm::{Error, Result}};
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

impl Storage for GlobalStorage {
    fn fetch_bytes(&self, key: &H256) -> Result<Vec<u8>> {
        let result = BACKEND
            .get(EkidenH256::from_str(&format!("{:x}", key)).unwrap())
            .wait();
        result.map_err(|err| Error::Storage(err.description().to_string()))
    }

    fn store_bytes(&mut self, bytes: &[u8]) -> Result<H256> {
        let result = BACKEND
            .insert(bytes.to_vec(), <u64>::max_value(), InsertOptions::default())
            .wait();
        match result {
            Ok(_) => Ok(H256::from_slice(&hash_storage_key(bytes).0)),
            Err(err) => Err(Error::Storage(err.description().to_string())),
        }
    }
}

/// Return the storage backend used by global storage.
#[cfg(not(target_env = "sgx"))]
pub fn get_storage_backend() -> Arc<StorageBackend> {
    BACKEND.clone()
}
