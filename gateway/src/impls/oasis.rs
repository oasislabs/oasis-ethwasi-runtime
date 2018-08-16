use client::Client;

use ekiden_common::bytes::H256;
use ekiden_common::futures::FutureExt;
use ekiden_core::futures::Future;
use ekiden_storage_base::{hash_storage_key, StorageBackend};
#[cfg(not(target_env = "sgx"))]
use ekiden_storage_dummy::DummyStorageBackend as StorageBackendImpl;
#[cfg(target_env = "sgx")]
use ekiden_trusted::db::untrusted::UntrustedStorageBackend as StorageBackendImpl;

use jsonrpc_core::{Error, ErrorCode, Result};

use parity_rpc::v1::types::H256 as RpcH256;

use traits::Oasis;

use std::str::FromStr;
use std::sync::Arc;

lazy_static! {
    static ref BACKEND: Arc<StorageBackend> = Arc::new(StorageBackendImpl::new());
}

/// Eth rpc implementation
pub struct OasisClient;

impl OasisClient {
    /// Creates new OasisClient.
    pub fn new() -> Self {
        OasisClient
    }
}

impl Oasis for OasisClient {
    fn request_bytes(&self, key: RpcH256) -> Result<String> {
        let result = BACKEND
            .get(H256::from_slice(&key.0))
            .wait();
        match result {
            Ok(data) => Ok(String::from_utf8(data).unwrap()),
            Err(err) => {
                let mut error = Error::new(ErrorCode::InternalError);
                error.message = err.description().to_string();
                Err(error)
            }
        }
    }

    fn store_bytes(&self, data: String, expiry: u64) -> Result<RpcH256> {
        let result = BACKEND.insert(data.clone().into_bytes(), expiry).wait();
        match result {
            Ok(_) => Ok(RpcH256::from_str(&format!("{:x}", hash_storage_key(data.as_bytes()))).unwrap()),
            Err(err) => {
                let mut error = Error::new(ErrorCode::InternalError);
                error.message = err.description().to_string();
                Err(error)
            }
        }
    }
}
