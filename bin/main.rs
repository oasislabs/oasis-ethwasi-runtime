//! Oasis runtime entry point.
extern crate oasis_runtime;

extern crate ethcore;
extern crate ethereum_types;
extern crate failure;
extern crate io_context;
extern crate oasis_core_keymanager_client;
extern crate oasis_core_runtime;
extern crate oasis_runtime_api;
extern crate oasis_runtime_common;
extern crate serde_bytes;

use std::sync::Arc;

use serde_bytes::ByteBuf;

use oasis_core_runtime::{
    common::version::Version, rak::RAK, register_runtime_txn_methods, version_from_cargo, Protocol,
    RpcDemux, RpcDispatcher, TxnDispatcher, TxnMethDispatcher,
};
use oasis_runtime::block::OasisBatchHandler;
use oasis_runtime_api::{with_api, ExecutionResult};
use oasis_runtime_keymanager::trusted_policy_signers;

fn main() {
    // Initializer.
    let init = |protocol: &Arc<Protocol>,
                rak: &Arc<RAK>,
                _rpc_demux: &mut RpcDemux,
                rpc: &mut RpcDispatcher|
     -> Option<Box<dyn TxnDispatcher>> {
        let mut txn = TxnMethDispatcher::new();
        {
            use oasis_runtime::methods::execute::*;
            with_api! { register_runtime_txn_methods!(txn, api); }
        }

        // Create the key manager client.
        let km_client = Arc::new(oasis_core_keymanager_client::RemoteClient::new_runtime(
            protocol.get_runtime_id(),
            protocol.clone(),
            rak.clone(),
            1024, // TODO: How big should this cache be?
            trusted_policy_signers(),
        ));
        let initializer_km_client = km_client.clone();

        #[cfg(not(target_env = "sgx"))]
        let _ = rpc;
        #[cfg(target_env = "sgx")]
        rpc.set_keymanager_policy_update_handler(Some(Box::new(move |raw_signed_policy| {
            km_client
                .set_policy(raw_signed_policy)
                .expect("failed to update km client policy");
        })));

        txn.set_batch_handler(OasisBatchHandler::new(initializer_km_client));
        Some(Box::new(txn))
    };

    // Start the runtime.
    oasis_core_runtime::start_runtime(Box::new(init), version_from_cargo!());
}
