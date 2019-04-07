use std::sync::Arc;

use ekiden_keymanager_client::{ContractId, KeyManagerClient};
use ekiden_runtime::common::logger::get_logger;
use ethereum_types::Address;
use futures::prelude::*;
use hash::keccak;
use io_context::Context;
use jsonrpc_core::{BoxFuture, Error, ErrorCode};
use jsonrpc_macros::Trailing;
use lazy_static::lazy_static;
use parity_rpc::v1::{
    helpers::errors,
    metadata::Metadata,
    types::{BlockNumber, Bytes, CallRequest, H160 as RpcH160},
};
use prometheus::{
    __register_counter_vec, histogram_opts, labels, opts, register_histogram_vec,
    register_int_counter_vec, HistogramVec, IntCounterVec,
};
use runtime_ethereum_api::TransactionRequest;
use slog::{info, Logger};

use crate::{
    client::Client,
    impls::eth::EthClient,
    traits::oasis::{Oasis, RpcPublicKeyPayload},
    util::execution_error,
};

// Metrics.
lazy_static! {
    static ref OASIS_RPC_CALLS: IntCounterVec = register_int_counter_vec!(
        "web3_gateway_oasis_rpc_calls",
        "Number of oasis API RPC calls",
        &["call"]
    )
    .unwrap();
    static ref OASIS_RPC_CALL_TIME: HistogramVec = register_histogram_vec!(
        "web3_gateway_oasis_rpc_call_time",
        "Time taken by oasis API RPC calls",
        &["call"],
        vec![0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 25.0, 50.0]
    )
    .unwrap();
}

/// Eth rpc implementation
pub struct OasisClient {
    logger: Logger,
    client: Arc<Client>,
    km_client: Arc<KeyManagerClient>,
}

impl OasisClient {
    /// Creates new OasisClient.
    pub fn new(client: Arc<Client>, km_client: Arc<KeyManagerClient>) -> Self {
        OasisClient {
            logger: get_logger("gateway/impls/oasis"),
            client,
            km_client,
        }
    }
}

impl Oasis for OasisClient {
    type Metadata = Metadata;

    fn public_key(&self, contract: Address) -> BoxFuture<Option<RpcPublicKeyPayload>> {
        OASIS_RPC_CALLS
            .with(&labels! {"call" => "publicKey",})
            .inc();

        info!(self.logger, "oasis_getPublicKey"; "contract" => ?contract);

        let contract_id = ContractId::from(&keccak(contract.to_vec())[..]);

        // TODO: Support proper I/O contexts (requires RPC interface changes).
        Box::new(
            self.km_client
                .get_public_key(Context::background(), contract_id)
                .map_err(move |err| errors::invalid_params(&contract.to_string(), err))
                .map(|maybe_payload| {
                    maybe_payload.map(|pk_payload| RpcPublicKeyPayload {
                        public_key: Bytes::from(pk_payload.key.as_ref().to_vec()),
                        timestamp: pk_payload.timestamp.unwrap_or(0),
                        signature: Bytes::from(pk_payload.signature.as_ref().to_vec()),
                    })
                }),
        )
    }

    fn call_enc(
        &self,
        _meta: Self::Metadata,
        request: CallRequest,
        tag: Trailing<BlockNumber>,
    ) -> BoxFuture<Bytes> {
        OASIS_RPC_CALLS.with(&labels! {"call" => "callEnc",}).inc();
        let timer = OASIS_RPC_CALL_TIME
            .with(&labels! {"call" => "callEnc",})
            .start_timer();
        let num = tag.unwrap_or_default();

        info!(self.logger, "oasis_call_enc"; "request" => ?request, "num" => ?num);

        let request = TransactionRequest {
            nonce: request.nonce.map(Into::into),
            caller: request.from.map(Into::into),
            is_call: request.to.is_some(),
            address: request.to.map(Into::into),
            input: request.data.map(Into::into),
            value: request.value.map(Into::into),
            gas: request.gas.map(Into::into),
        };

        Box::new(
            self.client
                .call_enc(request, EthClient::get_block_id(num))
                .map_err(execution_error)
                .map(Into::into)
                .then(move |result| {
                    drop(timer);
                    result
                }),
        )
    }

    fn get_expiry(&self, address: RpcH160, num: Trailing<BlockNumber>) -> BoxFuture<u64> {
        OASIS_RPC_CALLS
            .with(&labels! {"call" => "getExpiry",})
            .inc();
        let address: Address = RpcH160::into(address);
        let num = num.unwrap_or_default();

        info!(
            self.logger,
            "oasis_getExpiry";
                "address" => ?address,
                "num" => ?num
        );

        Box::new(
            self.client
                .storage_expiry(&address, EthClient::get_block_id(num))
                .map_err(|_| Error::new(ErrorCode::InternalError)),
        )
    }
}
