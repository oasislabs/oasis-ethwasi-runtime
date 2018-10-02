use ekiden_common::bytes::H256;
use ekiden_core::futures::Future;
use ekiden_storage_base::{hash_storage_key, InsertOptions, StorageBackend};

use jsonrpc_core::{Error, ErrorCode, Result};

use parity_rpc::v1::types::H256 as RpcH256;

use traits::Oasis;

use std::str::FromStr;
use std::sync::Arc;

/// Eth rpc implementation
pub struct OasisClient {
    storage: Arc<StorageBackend>,
}

impl OasisClient {
    /// Creates new OasisClient.
    pub fn new(storage: &Arc<StorageBackend>) -> Self {
        OasisClient {
            storage: storage.clone(),
        }
    }
}

impl Oasis for OasisClient {
    fn fetch_bytes(&self, key: RpcH256) -> Result<Vec<u8>> {
        let result = self.storage.get(H256::from_slice(&key.0)).wait();
        result.map_err(|err| {
            let mut error = Error::new(ErrorCode::InternalError);
            error.message = err.description().to_string();
            error
        })
    }

    fn store_bytes(&self, data: Vec<u8>, expiry: u64) -> Result<RpcH256> {
        let result = self.storage
            .insert(data.clone(), expiry, InsertOptions::default())
            .wait();
        match result {
            Ok(_) => Ok(RpcH256::from_str(&format!("{:x}", hash_storage_key(&data))).unwrap()),
            Err(err) => {
                let mut error = Error::new(ErrorCode::InternalError);
                error.message = err.description().to_string();
                Err(error)
            }
        }
    }
}
