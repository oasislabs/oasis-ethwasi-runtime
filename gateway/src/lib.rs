// Copyright 2015-2018 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! Ethcore client application.

#[macro_use]
extern crate clap;
extern crate futures;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate parking_lot;
extern crate rayon;
#[macro_use]
extern crate serde_derive;

extern crate jsonrpc_core;
#[macro_use]
extern crate jsonrpc_macros;
extern crate jsonrpc_http_server;
extern crate jsonrpc_pubsub;
extern crate jsonrpc_ws_server;

extern crate common_types;
extern crate ethcore;
extern crate ethcore_bytes as bytes;
extern crate ethcore_transaction as transaction;
extern crate ethereum_types;
#[cfg(test)]
extern crate hex;
extern crate keccak_hash as hash;
extern crate kvdb;
extern crate parity_reactor;
extern crate parity_rpc;
extern crate rlp_compress;

#[macro_use]
extern crate client_utils;
extern crate ekiden_common;
extern crate ekiden_core;
extern crate ekiden_db_trusted;
extern crate ekiden_runtime_client;
#[macro_use]
extern crate ekiden_instrumentation;
extern crate ekiden_keymanager_common;
extern crate ekiden_storage_base;
#[cfg(test)]
extern crate ekiden_storage_dummy;
extern crate ethereum_api;
#[cfg(test)]
extern crate grpcio;
extern crate runtime_ethereum_common;

extern crate ekiden_enclave_common;
extern crate ekiden_keymanager_client;

use ekiden_enclave_common::quote::MrEnclave;
use ekiden_keymanager_client::{KeyManager, NetworkRpcClientBackendConfig};

mod client;
mod impls;
mod informant;
mod middleware;
#[cfg(feature = "pubsub")]
mod notifier;
mod rpc;
mod rpc_apis;
mod run;
mod servers;
mod state;
#[cfg(test)]
mod test_helpers;
mod traits;
pub mod util;

use std::{sync::Arc, time::Duration};

use clap::ArgMatches;
use ethereum_types::U256;

use ekiden_core::{environment::Environment, x509};
use ekiden_runtime_client::create_runtime_client;
use ekiden_storage_base::BackendIdentityMapper;
use ethereum_api::with_api;

pub use self::run::RunningClient;

with_api! {
    create_runtime_client!(runtime_ethereum, ethereum_api, api);
}

pub fn start(
    args: ArgMatches,
    pubsub_interval_secs: u64,
    http_port: u16,
    num_threads: usize,
    ws_port: u16,
    ws_max_connections: usize,
    ws_rate_limit: usize,
    gas_price: U256,
    jsonrpc_max_batch_size: usize,
) -> Result<RunningClient, String> {
    let client = runtime_client!(runtime_ethereum, args);
    let environment = client.get_environment();
    let storage = client.get_storage();
    let roothash = client.get_roothash();

    let runtime_id = client_utils::args::get_runtime_id(&args);
    let snapshot_manager = client_utils::db::Manager::new(
        environment.clone(),
        runtime_id,
        roothash,
        Arc::new(BackendIdentityMapper::new(storage.clone())),
    );

    setup_key_manager(&args, environment.clone());

    run::execute(
        client,
        Some(snapshot_manager),
        storage,
        environment,
        pubsub_interval_secs,
        http_port,
        num_threads,
        ws_port,
        ws_max_connections,
        ws_rate_limit,
        gas_price,
        jsonrpc_max_batch_size,
    )
}

/// Configures the global KeyManager instance with the MRENCLAVE and
/// NetworkRpcClientBackendConfig specified by the cli args.
fn setup_key_manager(args: &ArgMatches, environment: Arc<Environment>) {
    let mut key_manager = KeyManager::instance().expect("Should always have a key manager");

    let backend = key_manager_backend(&args, environment);
    let mrenclave =
        value_t!(args.value_of("key-manager-mrenclave"), MrEnclave).unwrap_or_else(|e| e.exit());

    key_manager.configure_backend(backend);
    key_manager.set_contract(mrenclave);
}

fn key_manager_backend(
    args: &ArgMatches,
    environment: Arc<Environment>,
) -> NetworkRpcClientBackendConfig {
    let timeout = Some(Duration::new(5, 0));
    let host = value_t!(args.value_of("key-manager-host"), String).unwrap_or_else(|e| e.exit());
    let port = value_t!(args.value_of("key-manager-port"), u16).unwrap_or_else(|e| e.exit());
    let certificate = x509::load_certificate_pem(&args.value_of("key-manager-cert").unwrap())
        .expect("unable to load key manager's certificate");
    let certificate = x509::Certificate::from_pem(&certificate).unwrap();

    NetworkRpcClientBackendConfig {
        environment,
        timeout,
        host,
        port,
        certificate,
    }
}
