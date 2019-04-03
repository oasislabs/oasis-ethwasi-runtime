use jsonrpc_core::{futures::future, BoxFuture};
use parity_rpc::v1::{
    helpers::errors,
    metadata::Metadata,
    traits::EthSigning,
    types::{
        Bytes, RichRawTransaction, TransactionRequest, H160 as RpcH160, H256 as RpcH256,
        H520 as RpcH520,
    },
};

pub struct EthSigningClient {}

impl EthSigningClient {
    pub fn new() -> EthSigningClient {
        EthSigningClient {}
    }
}

impl EthSigning for EthSigningClient {
    type Metadata = Metadata;

    fn sign(&self, _: Metadata, _: RpcH160, _: Bytes) -> BoxFuture<RpcH520> {
        measure_counter_inc!("sign");
        Box::new(future::failed(errors::unsupported("eth_sign is not implemented because the gateway cannot sign transactions. \
            Make sure that the wallet is setup correctly in the client in case transaction signing is expected to happen transparently".to_string(), None)))
    }

    fn send_transaction(&self, _: Metadata, _: TransactionRequest) -> BoxFuture<RpcH256> {
        measure_counter_inc!("sendTransaction");
        Box::new(future::failed(errors::unsupported("eth_sendTransaction is not implemented because the gateway cannot sign transactions. \
            Make sure that the wallet is setup correctly in the client in case transaction signing is expected to happen transparently".to_string(), None)))
    }

    fn sign_transaction(
        &self,
        _: Metadata,
        _: TransactionRequest,
    ) -> BoxFuture<RichRawTransaction> {
        measure_counter_inc!("signTransaction");
        Box::new(future::failed(errors::unsupported("eth_signTransaction is not implemented because the gateway cannot sign transactions. \
            Make sure that the wallet is setup correctly in the client in case transaction signing is expected to happen transparently".to_string(), None)))
    }
}
