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

#![feature(use_extern_macros)]

extern crate ctrlc;
extern crate fdlimit;
#[macro_use]
extern crate log;
extern crate parking_lot;

extern crate web3_gateway;

// Ekiden client packages
#[macro_use]
extern crate clap;
extern crate grpcio;
extern crate rand;

#[macro_use]
extern crate client_utils;
extern crate ekiden_contract_client;
extern crate ekiden_core;
extern crate ekiden_rpc_client;

extern crate evm_api;

use clap::{App, Arg};
use ctrlc::CtrlC;
use fdlimit::raise_fd_limit;
use log::{error, info, log, warn, LevelFilter};
use parking_lot::{Condvar, Mutex};
use std::sync::Arc;

use ekiden_contract_client::create_contract_client;
use ekiden_core::{bytes::B256, ring::signature::Ed25519KeyPair, signature::InMemorySigner,
                  untrusted};
use evm_api::{with_api, AccountState, InitStateRequest};

use web3_gateway::start;

with_api! {
    create_contract_client!(evm, evm_api, api);
}

/// Generate client key pair.
fn create_key_pair() -> Arc<InMemorySigner> {
    let key_pair =
        Ed25519KeyPair::from_seed_unchecked(untrusted::Input::from(&B256::random())).unwrap();
    Arc::new(InMemorySigner::new(key_pair))
}

// Run our version of parity.
fn main() {
    // TODO: is this needed?
    // increase max number of open files
    raise_fd_limit();

    let known_components = client_utils::components::create_known_components();
    let args = default_app!()
        .args(&known_components.get_arguments())
        .arg(
            Arg::with_name("threads")
                .long("threads")
                .help("Number of threads to use for HTTP server.")
                .default_value("1")
                .takes_value(true),
        )
        .get_matches();

    // reset max log level to Info after default_app macro sets it to Trace
    log::set_max_level(LevelFilter::Info);

    // Initialize component container.
    let mut container = known_components
        .build_with_arguments(&args)
        .expect("failed to initialize component container");

    let signer = create_key_pair();
    let contract_client = contract_client!(signer, evm, args, container);

    let exit = Arc::new((Mutex::new(false), Condvar::new()));

    let client = start().unwrap();

    CtrlC::set_handler({
        let e = exit.clone();
        move || {
            e.1.notify_all();
        }
    });

    // Wait for signal
    let mut lock = exit.0.lock();
    let _ = exit.1.wait(&mut lock);

    client.shutdown();
}
