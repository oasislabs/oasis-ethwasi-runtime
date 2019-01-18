use std::str::FromStr;
use std::sync::Arc;

use ekiden_common::bytes::H256;
use ekiden_core::futures::Future;
use ekiden_storage_base::{hash_storage_key, InsertOptions, StorageBackend};

use ethereum_types::Address;
use jsonrpc_core::futures::future;
use jsonrpc_core::{BoxFuture, Error, ErrorCode, Result};
use jsonrpc_macros::Trailing;
use parity_rpc::v1::types::{BlockNumber, H160 as RpcH160, H256 as RpcH256};

use client::Client;
use impls::eth::EthClient;
use traits::Oasis;

/// Eth rpc implementation
pub struct OasisClient {
    client: Arc<Client>,
    storage: Arc<StorageBackend>,
}

impl OasisClient {
    /// Creates new OasisClient.
    pub fn new(client: Arc<Client>, storage: &Arc<StorageBackend>) -> Self {
        OasisClient {
            client: client,
            storage: storage.clone(),
        }
    }
}

impl Oasis for OasisClient {
    fn get_storage_expiry(&self, address: RpcH160, num: Trailing<BlockNumber>) -> BoxFuture<u64> {
        measure_counter_inc!("getStorageExpiry");
        let address: Address = RpcH160::into(address);
        let num = num.unwrap_or_default();

        info!(
            "oasis_getStorageExpiry(contract: {:?}, number: {:?})",
            address, num
        );
        Box::new(
            self.client
                .storage_expiry(&address, EthClient::get_block_id(num))
                .map_err(|_| Error::new(ErrorCode::InternalError)),
        )
    }

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
