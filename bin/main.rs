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
    common::{runtime::RuntimeId, version::Version},
    rak::RAK,
    register_runtime_txn_methods, version_from_cargo, Protocol, RpcDemux, RpcDispatcher,
    TxnDispatcher,
};
use oasis_runtime::block::OasisBatchHandler;
use oasis_runtime_api::{with_api, ExecutionResult};

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

        // Create the key manager client.
        let km_client = Arc::new(oasis_core_keymanager_client::RemoteClient::new_runtime(
            RuntimeId::default(), // HACK: This is what's deployed.
            protocol.clone(),
            rak.clone(),
            1024, // TODO: How big should this cache be?
        ));

        txn.set_batch_handler(OasisBatchHandler::new(km_client));
    };

    // Start the runtime.
    oasis_core_runtime::start_runtime(Some(Box::new(init)), version_from_cargo!());
}
