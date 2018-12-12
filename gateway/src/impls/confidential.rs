use client::Client;
use ekiden_common::bytes::B512;
use ekiden_core::futures::FutureExt;
use ekiden_keymanager_common::confidential;
use ethereum_api::TransactionRequest;
use ethereum_types::Address;
use impls::eth::EthClient;
use jsonrpc_core::futures::{future, Future};
use jsonrpc_core::{BoxFuture, Error, ErrorCode, Result};
use jsonrpc_macros::Trailing;
use parity_rpc::v1::helpers::errors;
use parity_rpc::v1::metadata::Metadata;
use parity_rpc::v1::traits::Eth;
use parity_rpc::v1::types::{BlockNumber, Bytes, CallRequest, H256};
use std::sync::Arc;
use traits::confidential::{Confidential, PublicKeyResult};

pub struct ConfidentialClient {
    client: Arc<Client>,
    eth_client: EthClient,
}

impl ConfidentialClient {
    pub fn new(client: Arc<Client>) -> Self {
        ConfidentialClient {
            client: client.clone(),
            eth_client: EthClient::new(&client),
        }
    }
}
impl Confidential for ConfidentialClient {
    type Metadata = Metadata;

    fn public_key(&self, contract: Address) -> Result<PublicKeyResult> {
        measure_counter_inc!("confidential_getPublicKey");
        info!("confidential_getPublicKey(contract {:?})", contract);
        self.client
            .public_key(contract)
            .map_err(|_| Error::new(ErrorCode::InternalError))
    }

    fn call_enc(
        &self,
        meta: Self::Metadata,
        request: CallRequest,
        tag: Trailing<BlockNumber>,
    ) -> BoxFuture<Bytes> {
        measure_counter_inc!("confidential_call");
        let num = tag.unwrap_or_default();
        info!(
            "confidential_call_enc(request: {:?}, number: {:?})",
            request, num
        );

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
            "confidential_call_enc_time",
            self.client
                .call_enc(request, EthClient::get_block_id(num))
                .map_err(errors::execution)
                .map(Into::into)
        ))
    }
}
