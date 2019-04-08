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

use std::sync::Arc;

use ekiden_runtime::{
    rak::RAK, register_runtime_txn_methods, Protocol, RpcDispatcher, TxnDispatcher,
};
use ethereum_types::{Address, H256, U256};
#[cfg(target_env = "sgx")]
use runtime_ethereum::KM_ENCLAVE_HASH;
use runtime_ethereum::{cache::Cache, EthereumBatchHandler};
use runtime_ethereum_api::{
    with_api, BlockId, ExecuteTransactionResponse, Filter, Log, Receipt,
    SimulateTransactionResponse, Transaction, TransactionRequest,
};

fn main() {
    // Initializer.
    let init = |protocol: &Arc<Protocol>,
                rak: &Arc<RAK>,
                _rpc: &mut RpcDispatcher,
                txn: &mut TxnDispatcher| {
        {
            use runtime_ethereum::methods::*;
            with_api! { register_runtime_txn_methods!(txn, api); }
        }

        // Create the key manager client.
        let km_client = Arc::new(ekiden_keymanager_client::RemoteClient::new_runtime(
            #[cfg(target_env = "sgx")]
            Some(KM_ENCLAVE_HASH),
            #[cfg(not(target_env = "sgx"))]
            None,
            protocol.clone(),
            rak.clone(),
        ));

        // Create the global Parity blockchain cache.
        let cache = Arc::new(Cache::new(km_client));

        txn.set_batch_handler(EthereumBatchHandler::new(cache.clone()));
        txn.set_finalizer(move |new_state_root| cache.finalize_root(new_state_root));
    };

    // Start the runtime.
    ekiden_runtime::start_runtime(Some(Box::new(init)));
}
