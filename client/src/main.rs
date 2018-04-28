#![feature(use_extern_macros)]

// Sputnik/Ethereum packages

extern crate sputnikvm_network_classic;
extern crate sputnikvm_network_ellaism;
extern crate sputnikvm_network_expanse;
extern crate sputnikvm_network_foundation;
extern crate sputnikvm_network_musicoin;
extern crate sputnikvm_network_ubiq;

extern crate bigint;
extern crate block;
extern crate blockchain;
extern crate bloom;
extern crate env_logger;
extern crate hexutil;
extern crate jsonrpc_core;
extern crate jsonrpc_http_server;
#[macro_use]
extern crate jsonrpc_macros;
extern crate lazy_static;
extern crate log;
extern crate rlp;
extern crate secp256k1;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate sha3;
extern crate sputnikvm;
extern crate sputnikvm_stateful;
extern crate trie;

extern crate hex;
extern crate tokio_core;

mod error;
mod rpc;

use std::sync::{Arc, Mutex};

use sputnikvm_network_foundation::ByzantiumPatch;

// Ekiden client packages

#[macro_use]
extern crate clap;
extern crate futures;
extern crate grpcio;
extern crate rand;

#[macro_use]
extern crate client_utils;
extern crate ekiden_core;
extern crate ekiden_rpc_client;

extern crate evm_api;

use clap::{App, Arg};
use futures::future::Future;

use ekiden_rpc_client::create_client_rpc;
use evm_api::{with_api, InitStateRequest};

with_api! {
    create_client_rpc!(evm, evm_api, api);
}

fn main() {
    let mut client = contract_client!(evm);

    println!("Initializing genesis state");
    client
        .init_genesis_state(evm::InitStateRequest::new())
        .wait()
        .unwrap();

    env_logger::init();

    let client_arc = Arc::new(Mutex::new(client));
    let addr = "0.0.0.0:8545".parse().unwrap();

    println!("Started RPC server");

    /*
    thread::spawn(move || {
        miner::mine_loop::<ByzantiumPatch, ekiden_rpc_client::backend::Web3ContractClientBackend>(&mut client, miner_arc, receiver);
    });
    */

    rpc::rpc_loop::<ByzantiumPatch>(client_arc, &addr);
}
