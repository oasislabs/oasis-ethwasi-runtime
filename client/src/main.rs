#![feature(use_extern_macros)]

// Sputnik/Ethereum packages
extern crate sputnikvm_network_foundation;

extern crate bigint;
extern crate block;
extern crate blockchain;
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

extern crate hex;

mod error;
mod rpc;

use sputnikvm::Patch;
use sputnikvm_network_foundation::ByzantiumPatch;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver, Sender};

// Ekiden client packages

#[macro_use]
extern crate clap;
extern crate futures;
extern crate grpcio;
extern crate rand;

#[macro_use]
extern crate client_utils;
extern crate ekiden_contract_client;
extern crate ekiden_core;
extern crate ekiden_rpc_client;

extern crate evm_api;

use clap::{App, Arg};
use futures::future::Future;
use std::fs;

use ekiden_contract_client::create_contract_client;
use ekiden_core::bytes::B256;
use ekiden_core::ring::signature::Ed25519KeyPair;
use ekiden_core::signature::InMemorySigner;
use ekiden_core::untrusted;
use evm_api::{with_api, AccountState, InitStateRequest};

with_api! {
    create_contract_client!(evm, evm_api, api);
}

/// Generate client key pair.
fn create_key_pair() -> Arc<InMemorySigner> {
    let key_pair =
        Ed25519KeyPair::from_seed_unchecked(untrusted::Input::from(&B256::random())).unwrap();
    Arc::new(InMemorySigner::new(key_pair))
}

#[derive(Serialize, Deserialize, Debug)]
struct Account {
    nonce: String,
    balance: String,
    storage: HashMap<String, String>,
    code: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct AccountMap {
    accounts: HashMap<String, Account>,
}

fn main() {
    let signer = create_key_pair();
    let mut client = contract_client!(signer, evm);

    let is_genesis_initialized = client.genesis_block_initialized(true).wait().unwrap();
    if is_genesis_initialized {
        println!("Genesis block already initialized");
    } else {
        init_genesis_block(&mut client);
    }

    let client_arc = Arc::new(Mutex::new(client));
    let addr = "0.0.0.0:8545".parse().unwrap();

    rpc::rpc_loop::<ByzantiumPatch>(client_arc, &addr);
}

fn init_genesis_block(client: &mut evm::Client<ekiden_rpc_client::backend::Web3RpcClientBackend>) {
    println!("Initializing genesis block");
    let mut init_state_request = evm::InitStateRequest::new();

    // Read in all the files in resources/genesis/
    for path in fs::read_dir("../resources/genesis").unwrap() {
        let path = path.unwrap().path();
        let br = BufReader::new(File::open(path.clone()).unwrap());

        // Parse the JSON file.
        let accounts: AccountMap = serde_json::from_reader(br).unwrap();
        println!(
            "  {:?} -> {} accounts",
            path.file_name().unwrap(),
            accounts.accounts.len()
        );

        for (addr, account) in accounts.accounts {
            let mut account_state = AccountState::new();
            account_state.set_nonce(account.nonce);
            account_state.set_address(addr);
            account_state.set_balance(account.balance);
            if account.code != "0x" {
                account_state.set_code(account.code);
            }
            for (key, value) in account.storage {
                account_state.storage.insert(key, value);
            }
            init_state_request.accounts.push(account_state);
        }
    }

    let result = client
        .init_genesis_block(init_state_request)
        .wait()
        .unwrap();
    println!("  {:?}", result);
}
