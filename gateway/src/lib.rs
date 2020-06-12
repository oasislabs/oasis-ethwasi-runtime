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

//! Web3 gateway.

extern crate clap;
extern crate futures;
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate parking_lot;
#[macro_use]
extern crate serde_derive;
extern crate jsonrpc_core;
#[macro_use]
extern crate jsonrpc_macros;
extern crate anyhow;
extern crate ethcore;
extern crate ethereum_types;
extern crate grpcio;
#[cfg(test)]
extern crate hex;
extern crate io_context;
extern crate jsonrpc_http_server;
extern crate jsonrpc_pubsub;
extern crate jsonrpc_ws_server;
extern crate keccak_hash as hash;
extern crate parity_reactor;
extern crate parity_rpc;
extern crate prometheus;
extern crate serde_bytes;
extern crate slog;
extern crate tokio;
extern crate tokio_threadpool;

extern crate oasis_core_client;
extern crate oasis_core_keymanager_client;
extern crate oasis_core_runtime;

extern crate oasis_runtime_api;
extern crate oasis_runtime_common;

mod impls;
mod informant;
mod middleware;
mod pubsub;
mod rpc;
mod rpc_apis;
mod run;
mod servers;
mod traits;
mod translator;
pub mod util;

use std::sync::Arc;

use anyhow::Result;
use clap::{value_t_or_exit, ArgMatches};
use ethereum_types::U256;
use grpcio::EnvBuilder;
use oasis_core_client::{create_txn_api_client, Node, TxnClient};
use oasis_core_runtime::common::runtime::RuntimeId;
use oasis_runtime_api::*;
use serde_bytes::ByteBuf;

pub use self::run::RunningGateway;

with_api! {
    create_txn_api_client!(EthereumRuntimeClient, api);
}

pub fn start(
    args: ArgMatches,
    pubsub_interval_secs: u64,
    interface: &str,
    http_port: u16,
    num_threads: usize,
    ws_port: u16,
    ws_max_connections: usize,
    ws_rate_limit: usize,
    gas_price: U256,
    jsonrpc_max_batch_size: usize,
) -> Result<RunningGateway> {
    let node_address = args.value_of("node-address").unwrap();
    let runtime_id = value_t_or_exit!(args, "runtime-id", RuntimeId);

    let env = Arc::new(EnvBuilder::new().build());
    let node = Node::new(env.clone(), node_address);
    let txn_client = TxnClient::new(node.channel(), runtime_id, None);
    let client = EthereumRuntimeClient::new(txn_client);
    // TODO: Key manager MRENCLAVE.
    let km_client = Arc::new(oasis_core_keymanager_client::RemoteClient::new_grpc(
        runtime_id,
        None,
        node.channel(),
        1024, // TODO: How big should this cache be?
    ));

    run::execute(
        client,
        km_client,
        pubsub_interval_secs,
        interface,
        http_port,
        num_threads,
        ws_port,
        ws_max_connections,
        ws_rate_limit,
        gas_price,
        jsonrpc_max_batch_size,
    )
}
