use ekiden_common::bytes::H256 as EkidenH256;
use ekiden_common::futures::FutureExt;
use ekiden_core::futures::Future;
use ekiden_storage_base::{hash_storage_key, StorageBackend};
use ethcore::storage::Storage;
use vm::{Error, Result};
use ethereum_types::H256;

use std::str::FromStr;
use std::sync::Arc;

pub struct Web3GlobalStorage{
    backend: Arc<StorageBackend>,
}

impl Web3GlobalStorage {
    pub fn new(backend: Arc<StorageBackend>) -> Self {
        Web3GlobalStorage {
            backend: backend,
        }
    }
}

impl Storage for Web3GlobalStorage {
    fn request_bytes(&mut self, key: H256) -> Result<Vec<u8>> {
        let result = self.backend
            .get(EkidenH256::from_str(&format!("{:x}", key)).unwrap())
            .wait();
        result.map_err(|err| Error::Storage(err.description().to_string()))
    }

    fn store_bytes(&mut self, bytes: &[u8]) -> Result<H256> {
        let result = self.backend.insert(bytes.to_vec(), <u64>::max_value()).wait();
        match result {
            Ok(_) => Ok(H256::from_slice(&hash_storage_key(bytes).0)),
            Err(err) => Err(Error::Storage(err.description().to_string())),
        }
    }
}
