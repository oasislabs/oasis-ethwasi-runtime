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

#![feature(int_to_from_bytes)]

#[macro_use]
extern crate clap;
extern crate env_logger;
#[macro_use]
extern crate futures;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate parking_lot;
extern crate path;
extern crate rayon;
extern crate regex;
extern crate rustc_hex;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate toml;

extern crate jsonrpc_core;
#[macro_use]
extern crate jsonrpc_macros;
extern crate jsonrpc_http_server;
extern crate jsonrpc_ipc_server;
extern crate jsonrpc_pubsub;
extern crate jsonrpc_ws_server;

extern crate common_types;
#[macro_use]
extern crate ethcore;
extern crate ethcore_bytes as bytes;
extern crate ethcore_transaction as transaction;
extern crate ethereum_types;
extern crate evm;
#[cfg(test)]
extern crate hex;
extern crate journaldb;
extern crate keccak_hash as hash;
extern crate kvdb;
extern crate parity_machine;
extern crate parity_reactor;
extern crate parity_rpc;
extern crate rlp;
extern crate rlp_compress;
extern crate util_error;
extern crate vm;

#[macro_use]
extern crate client_utils;
extern crate ekiden_common;
extern crate ekiden_core;
extern crate ekiden_db_trusted;
extern crate ekiden_di;
extern crate ekiden_runtime_client;
#[macro_use]
extern crate ekiden_instrumentation;
extern crate ekiden_keymanager_common;
#[cfg(test)]
extern crate ekiden_registry_client;
#[cfg(test)]
extern crate ekiden_roothash_client;
extern crate ekiden_rpc_client;
#[cfg(test)]
extern crate ekiden_scheduler_client;
extern crate ekiden_storage_base;
#[cfg(test)]
extern crate ekiden_storage_dummy;
#[cfg(test)]
extern crate ekiden_storage_frontend;
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
mod storage;
#[cfg(test)]
mod test_helpers;
mod traits;
pub mod util;

use std::sync::Arc;
use std::time::Duration;

use clap::ArgMatches;
use ethereum_types::U256;

use ekiden_core::{environment::Environment, x509};
use ekiden_di::Container;
use ekiden_runtime_client::create_runtime_client;
use ekiden_storage_base::StorageBackend;
use ethereum_api::with_api;

pub use self::run::RunningClient;

with_api! {
    create_runtime_client!(runtime_ethereum, ethereum_api, api);
}

pub fn start(
    args: ArgMatches,
    mut container: Container,
    pubsub_interval_secs: u64,
    http_port: u16,
    num_threads: usize,
    ws_port: u16,
    ws_max_connections: usize,
    ws_rate_limit: usize,
    gas_price: U256,
    jsonrpc_max_batch_size: usize,
) -> Result<RunningClient, String> {
    let client = runtime_client!(runtime_ethereum, args, container);
    let storage: Arc<StorageBackend> = container
        .inject()
        .map_err(|err| err.description().to_string())?;
    let environment: Arc<Environment> = container
        .inject()
        .map_err(|err| err.description().to_string())?;

    let runtime_id = client_utils::args::get_runtime_id(&args);
    let snapshot_manager =
        client_utils::db::Manager::new_from_injected(runtime_id, &mut container).unwrap();

    setup_key_manager(&args, &mut container);

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
fn setup_key_manager(args: &ArgMatches, container: &mut Container) {
    let mut key_manager = KeyManager::instance().expect("Should always have a key manager");

    let backend = key_manager_backend(&args, container);
    let mrenclave =
        value_t!(args.value_of("key-manager-mrenclave"), MrEnclave).unwrap_or_else(|e| e.exit());

    key_manager.configure_backend(backend);
    key_manager.set_contract(mrenclave);
}

fn key_manager_backend(
    args: &ArgMatches,
    container: &mut Container,
) -> NetworkRpcClientBackendConfig {
    let environment = container.inject::<Environment>().unwrap().clone();
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
