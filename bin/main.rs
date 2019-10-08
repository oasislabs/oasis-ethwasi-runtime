//! Oasis runtime entry point.
extern crate oasis_runtime;

extern crate ekiden_keymanager_client;
extern crate ekiden_runtime;
extern crate ethcore;
extern crate ethereum_types;
extern crate failure;
extern crate io_context;
extern crate oasis_runtime_api;
extern crate oasis_runtime_common;
extern crate serde_bytes;

use std::sync::Arc;

use serde_bytes::ByteBuf;

use ekiden_runtime::{
    common::{runtime::RuntimeId, version::Version},
    rak::RAK,
    register_runtime_txn_methods, version_from_cargo, Protocol, RpcDemux, RpcDispatcher,
    TxnDispatcher,
};
use oasis_runtime::block::OasisBatchHandler;
#[cfg(target_env = "sgx")]
use oasis_runtime::KM_ENCLAVE_HASH;
use oasis_runtime_api::{with_api, ExecutionResult};

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
            use oasis_runtime::methods::execute::*;
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

        txn.set_batch_handler(OasisBatchHandler::new(km_client));
    };

    // Start the runtime.
    ekiden_runtime::start_runtime(Some(Box::new(init)), version_from_cargo!());
}
