//! Ethereum runtime entry point.
extern crate runtime_ethereum;

extern crate ekiden_keymanager_client;
extern crate ekiden_runtime;
extern crate ethcore;
extern crate ethereum_types;
extern crate failure;
extern crate io_context;
extern crate runtime_ethereum_api;
extern crate runtime_ethereum_common;
extern crate serde_bytes;

use std::sync::Arc;

use serde_bytes::ByteBuf;

use ekiden_runtime::{
    common::runtime::RuntimeId, rak::RAK, register_runtime_txn_methods, Protocol, RpcDemux,
    RpcDispatcher, TxnDispatcher,
};
use runtime_ethereum::block::EthereumBatchHandler;
#[cfg(target_env = "sgx")]
use runtime_ethereum::KM_ENCLAVE_HASH;
use runtime_ethereum_api::{with_api, ExecutionResult};

#[cfg(target_env = "sgx")]
use ekiden_runtime::common::sgx::avr::EnclaveIdentity;
#[cfg(target_env = "sgx")]
use std::collections::HashSet;

fn main() {
    // Initializer.
    let init = |protocol: &Arc<Protocol>,
                rak: &Arc<RAK>,
                _rpc_demux: &mut RpcDemux,
                _rpc: &mut RpcDispatcher,
                txn: &mut TxnDispatcher| {
        {
            use runtime_ethereum::methods::execute::*;
            with_api! { register_runtime_txn_methods!(txn, api); }
        }

        #[cfg(target_env = "sgx")]
        let remote_enclaves: Option<HashSet<EnclaveIdentity>> = Some(
            [EnclaveIdentity::fortanix_test(KM_ENCLAVE_HASH)]
                .iter()
                .cloned()
                .collect(),
        );
        #[cfg(not(target_env = "sgx"))]
        let remote_enclaves = None;

        // Create the key manager client.
        let km_client = Arc::new(ekiden_keymanager_client::RemoteClient::new_runtime(
            RuntimeId::default(), // HACK: This is what's deployed.
            remote_enclaves,
            protocol.clone(),
            rak.clone(),
            1024, // TODO: How big should this cache be?
        ));

        txn.set_batch_handler(EthereumBatchHandler::new(km_client));
    };

    // Start the runtime.
    ekiden_runtime::start_runtime(Some(Box::new(init)));
}
