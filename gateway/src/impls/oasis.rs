use std::sync::Arc;

use ekiden_core::futures::Future;

use ethereum_api::TransactionRequest;
use ethereum_types::Address;
use jsonrpc_core::{BoxFuture, Error, ErrorCode, Result};
use jsonrpc_macros::Trailing;
use parity_rpc::v1::{
    helpers::errors,
    metadata::Metadata,
    types::{BlockNumber, Bytes, CallRequest, H160 as RpcH160},
};

use client::Client;
use impls::eth::EthClient;
use traits::oasis::{Oasis, PublicKeyResult};

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
    type Metadata = Metadata;

    fn public_key(&self, contract: Address) -> Result<Option<PublicKeyResult>> {
        measure_counter_inc!("oasis_getPublicKey");
        info!("oasis_getPublicKey(contract {:?})", contract);
        self.client
            .public_key(contract)
            .map_err(|_| Error::new(ErrorCode::InternalError))
    }

    fn call_enc(
        &self,
        _meta: Self::Metadata,
        request: CallRequest,
        tag: Trailing<BlockNumber>,
    ) -> BoxFuture<Bytes> {
        measure_counter_inc!("oasis_call_enc");
        let num = tag.unwrap_or_default();
        info!("oasis_call_enc(request: {:?}, number: {:?})", request, num);

        let request = TransactionRequest {
            nonce: request.nonce.map(Into::into),
            caller: request.from.map(Into::into),
            is_call: request.to.is_some(),
            address: request.to.map(Into::into),
            input: request.data.map(Into::into),
            value: request.value.map(Into::into),
            gas: request.gas.map(Into::into),
        };
        Box::new(measure_future_histogram_timer!(
            "oasis_call_enc_time",
            self.client
                .call_enc(request, EthClient::get_block_id(num))
                .map_err(errors::execution)
                .map(Into::into)
        ))
    }

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
