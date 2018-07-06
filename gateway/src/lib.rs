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

#[macro_use]
extern crate client_utils;
extern crate ekiden_contract_client;
extern crate ekiden_core;
extern crate ekiden_di;
extern crate ekiden_rpc_client;
extern crate evm_api;

use std::{collections::HashMap,
          fs::{self, File},
          io::BufReader,
          str::FromStr,
          sync::Arc};

use clap::ArgMatches;
use ethereum_types::{Address, H256, U256};
use futures::future::Future;

use ekiden_contract_client::create_contract_client;
use ekiden_core::{bytes::B256, ring::signature::Ed25519KeyPair, signature::InMemorySigner,
                  untrusted};
use ekiden_di::Container;
use evm_api::{with_api, AccountState, InitStateRequest};

pub use self::run::RunningClient;

with_api! {
    create_contract_client!(runtime_evm, evm_api, api);
}

/// Generate client key pair.
fn create_key_pair() -> Arc<InMemorySigner> {
    let key_pair =
        Ed25519KeyPair::from_seed_unchecked(untrusted::Input::from(&B256::random())).unwrap();
    Arc::new(InMemorySigner::new(key_pair))
}

fn strip_0x<'a>(hex: &'a str) -> &'a str {
    if hex.starts_with("0x") {
        hex.get(2..).unwrap()
    } else {
        hex
    }
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

pub fn start(
    args: ArgMatches,
    mut container: Container,
    num_threads: usize,
) -> Result<RunningClient, String> {
    let signer = create_key_pair();
    let client = contract_client!(signer, runtime_evm, args, container);

    let is_genesis_initialized = client.genesis_block_initialized(true).wait().unwrap();
    if is_genesis_initialized {
        warn!("Genesis block already initialized");
    } else {
        init_genesis_block(&client);
    }

    run::execute(client, num_threads)
}

fn init_genesis_block(client: &runtime_evm::Client) {
    info!("Initializing genesis block");
    let mut account_request = Vec::new();
    let mut storage_request = Vec::new();

    // Read in all the files in resources/genesis/
    for path in fs::read_dir("../resources/genesis").unwrap() {
        let path = path.unwrap().path();
        let br = BufReader::new(File::open(path.clone()).unwrap());

        // Parse the JSON file.
        let accounts: AccountMap = serde_json::from_reader(br).unwrap();
        info!(
            "  {:?} -> {} accounts",
            path.file_name().unwrap(),
            accounts.accounts.len()
        );

        for (mut addr, account) in accounts.accounts {
            let address = Address::from_str(strip_0x(&addr)).unwrap();

            let mut account_state = AccountState {
                nonce: U256::from_dec_str(&account.nonce).unwrap(),
                address: address,
                balance: U256::from_dec_str(&account.balance).unwrap(),
                code: if account.code == "0x" {
                    String::new()
                } else {
                    account.code
                },
            };

            for (key, value) in account.storage {
                storage_request.push((
                    address,
                    H256::from_str(&key).unwrap(),
                    H256::from_str(&value).unwrap(),
                ));
            }

            account_request.push(account_state);
        }
    }
    let result = client.inject_accounts(account_request).wait().unwrap();
    let result = client
        .inject_account_storage(storage_request)
        .wait()
        .unwrap();

    let init_state_request = InitStateRequest {};
    let result = client
        .init_genesis_block(init_state_request)
        .wait()
        .unwrap();
}
