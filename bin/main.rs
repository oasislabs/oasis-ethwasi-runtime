//! Ethereum runtime entry point.
extern crate runtime_ethereum;

extern crate ekiden_keymanager_client;
extern crate ekiden_runtime;
extern crate ethcore;
extern crate ethereum_types;
extern crate failure;
extern crate io_context;
extern crate lazy_static;
extern crate runtime_ethereum_api;
extern crate runtime_ethereum_common;
extern crate serde_bytes;

use std::sync::Arc;

use serde_bytes::ByteBuf;

use ekiden_runtime::{
    common::runtime::RuntimeId, rak::RAK, register_runtime_txn_methods, Protocol, RpcDispatcher,
    TxnDispatcher,
};
use runtime_ethereum::block::EthereumBatchHandler;
#[cfg(target_env = "sgx")]
use runtime_ethereum::KM_ENCLAVE_HASH;
use runtime_ethereum_api::{with_api, ExecutionResult};

fn main() {
    // Initializer.
    let init = |protocol: &Arc<Protocol>,
                rak: &Arc<RAK>,
                _rpc: &mut RpcDispatcher,
                txn: &mut TxnDispatcher| {
        {
            use runtime_ethereum::methods::execute::*;
            with_api! { register_runtime_txn_methods!(txn, api); }
        }

        // Create the key manager client.
        let km_client = Arc::new(ekiden_keymanager_client::RemoteClient::new_runtime(
            RuntimeId::default(), // HACK: This is what's deployed.
            #[cfg(target_env = "sgx")]
            Some(KM_ENCLAVE_HASH),
            #[cfg(not(target_env = "sgx"))]
            None,
            protocol.clone(),
            rak.clone(),
            1024, // TODO: How big should this cache be?
        ));

        txn.set_batch_handler(EthereumBatchHandler::new(km_client));
    };

    // Start the runtime.
    ekiden_runtime::start_runtime(Some(Box::new(init)));
}
