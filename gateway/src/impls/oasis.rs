use std::{str::FromStr, sync::Arc};

use ekiden_common::bytes::H256;
use ekiden_core::futures::Future;

use ethereum_types::Address;
use jsonrpc_core::{futures::future, BoxFuture, Error, ErrorCode, Result};
use jsonrpc_macros::Trailing;
use parity_rpc::v1::types::{BlockNumber, H160 as RpcH160, H256 as RpcH256};

use client::Client;
use impls::eth::EthClient;
use traits::Oasis;

/// Eth rpc implementation
pub struct OasisClient {
    client: Arc<Client>,
}

impl OasisClient {
    /// Creates new OasisClient.
    pub fn new(client: Arc<Client>) -> Self {
        OasisClient { client: client }
    }
}

impl Oasis for OasisClient {
    fn get_expiry(&self, address: RpcH160, num: Trailing<BlockNumber>) -> BoxFuture<u64> {
        measure_counter_inc!("getExpiry");
        let address: Address = RpcH160::into(address);
        let num = num.unwrap_or_default();

        info!(
            "oasis_getExpiry(contract: {:?}, number: {:?})",
            address, num
        );
        Box::new(
            self.client
                .storage_expiry(&address, EthClient::get_block_id(num))
                .map_err(|_| Error::new(ErrorCode::InternalError)),
        )
    }
}
