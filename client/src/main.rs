#![feature(use_extern_macros)]

// Sputnik/Ethereum packages

extern crate sputnikvm_network_classic;
extern crate sputnikvm_network_foundation;
extern crate sputnikvm_network_ubiq;
extern crate sputnikvm_network_ellaism;
extern crate sputnikvm_network_expanse;
extern crate sputnikvm_network_musicoin;

extern crate sputnikvm;
extern crate sputnikvm_stateful;
extern crate secp256k1;
extern crate sha3;
extern crate blockchain;
extern crate bigint;
extern crate rlp;
extern crate bloom;
extern crate block;
extern crate trie;
extern crate hexutil;
#[macro_use]
extern crate lazy_static;
extern crate jsonrpc_core;
extern crate jsonrpc_http_server;
#[macro_use]
extern crate jsonrpc_macros;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
extern crate env_logger;

extern crate hex;
extern crate tokio_core;

mod error;
mod miner;
mod rpc;

use miner::MinerState;
use rand::os::OsRng;
use secp256k1::key::{PublicKey, SecretKey};
use secp256k1::SECP256K1;
use bigint::U256;
use hexutil::*;
use std::thread;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Sender, Receiver};
use sputnikvm::Patch;

use sputnikvm_network_foundation::ByzantiumPatch;


// Ekiden client packages

#[macro_use]
extern crate clap;
extern crate futures;
extern crate rand;
extern crate grpcio;

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
    let mut call_client = contract_client!(evm);

    println!("Initializing genesis state");
    client
        .init_genesis_state(evm::InitStateRequest::new())
        .wait()
        .unwrap();

    env_logger::init();

    // Initial account for testing. Hardcoded here for simplicity; we should move this to separate config.
    let private_key = "C87509A1C067BBDE78BEB793E6FA76530B6382A4C0241E5E4A9EC0A0F44DC0D3";
    let balance = "100000000000000000000";
    let chain = "foundation";
    let addr = "127.0.0.1:8545".parse().unwrap();

    let mut rng = OsRng::new().unwrap();

    let secret_key = SecretKey::from_slice(&SECP256K1, &read_hex(private_key).unwrap()).unwrap();

    let balance = U256::from_dec_str(balance).unwrap();

    let mut genesis = Vec::new();
    genesis.push((secret_key, balance));

    // add account address 7110316b618d20d0c44728ac2a3d683536ea682b. TODO: move this to a genesis config file
    genesis.push((SecretKey::from_slice(&SECP256K1, &read_hex("533d62aea9bbcb821dfdda14966bb01bfbbb53b7e9f5f0d69b8326e052e3450c").unwrap()).unwrap(), U256::from_dec_str("200000000000000000000").unwrap()));

    let (sender, receiver) = channel::<bool>();

    let state = miner::make_state::<ByzantiumPatch>(genesis);

    let client_arc = Arc::new(Mutex::new(call_client));

    let miner_arc = Arc::new(Mutex::new(state));
    let rpc_arc = miner_arc.clone();

    println!("Started RPC server");

    thread::spawn(move || {
        miner::mine_loop::<ByzantiumPatch, ekiden_rpc_client::backend::Web3ContractClientBackend>(&mut client, miner_arc, receiver);
    });

    rpc::rpc_loop::<ByzantiumPatch>(client_arc, rpc_arc, &addr, sender);
}
