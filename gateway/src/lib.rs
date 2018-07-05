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
extern crate env_logger;
#[macro_use]
extern crate futures;
extern crate futures_cpupool;
extern crate jsonrpc_core;

#[macro_use]
extern crate jsonrpc_macros;
extern crate jsonrpc_http_server;
extern crate jsonrpc_ipc_server;
extern crate jsonrpc_pubsub;
extern crate jsonrpc_ws_server;

extern crate parking_lot;
extern crate regex;
extern crate rlp;
extern crate rustc_hex;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate toml;

#[macro_use]
extern crate ethcore;
extern crate ethcore_bytes as bytes;
extern crate ethcore_transaction as transaction;
extern crate ethereum_types;
//extern crate kvdb;
extern crate journaldb;
extern crate keccak_hash as hash;
extern crate parity_reactor;
extern crate parity_rpc;
extern crate path;
extern crate registrar;

// for client.rs
extern crate common_types;
extern crate evm;
extern crate parity_machine;
extern crate util_error;
extern crate vm;

#[macro_use]
extern crate log as rlog;

mod client;
mod impls;
mod rpc;
mod rpc_apis;
mod run;
mod servers;

pub use self::run::RunningClient;

pub fn start() -> Result<RunningClient, String> {
    run::execute()
}
