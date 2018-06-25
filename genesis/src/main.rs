#![feature(use_extern_macros)]
use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;

extern crate clap;
use clap::{crate_authors, crate_description, crate_name, crate_version, value_t_or_exit, App, Arg};
extern crate ethereum_types;
use ethereum_types::{Address, H256, U256};
extern crate filebuffer;
extern crate futures;
use futures::future::Future;
extern crate grpcio;
extern crate hex;
extern crate log;
use log::{debug, log};
extern crate pretty_env_logger;
extern crate rlp;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

extern crate client_utils;
use client_utils::{contract_client, default_app};
extern crate ekiden_contract_client;
use ekiden_contract_client::create_contract_client;
extern crate ekiden_core;
use ekiden_core::bytes::B256;
extern crate ekiden_rpc_client;

extern crate evm_api;
use evm_api::{with_api, AccountState};

use std::time::{Duration, Instant};

with_api! {
    create_contract_client!(evm, evm_api, api);
}

/// When restoring an exported state, inject this many accounts at a time.
const INJECT_CHUNK_SIZE: usize = 100;
/// When restoring an exported state, inject this many account storage items at a time.
const INJECT_STORAGE_CHUNK_SIZE: usize = 1000;

#[derive(Deserialize)]
struct ExportedAccount {
    balance: String,
    nonce: String,
    code: Option<String>,
    storage: Option<HashMap<String, String>>,
}
#[derive(Deserialize)]
struct ExportedState {
    state: BTreeMap<String, ExportedAccount>,
}

fn to_ms(d: Duration) -> f64 {
    d.as_secs() as f64 * 1e3 + d.subsec_nanos() as f64 * 1e-6
}

fn strip_0x<'a>(hex: &'a str) -> &'a str {
    if hex.starts_with("0x") {
        hex.get(2..).unwrap()
    } else {
        hex
    }
}

fn main() {
    let seed = ekiden_core::bytes::B256::random();
    let seed_input = ekiden_core::untrusted::Input::from(&seed);
    let key_pair =
        ekiden_core::ring::signature::Ed25519KeyPair::from_seed_unchecked(seed_input).unwrap();
    let signer = std::sync::Arc::new(ekiden_core::signature::InMemorySigner::new(key_pair));
    let known_components = client_utils::components::create_known_components();
    let args = default_app!()
        .args(&known_components.get_arguments())
        .arg(
            Arg::with_name("exported_state")
                .takes_value(true)
                .required(true),
        )
        .get_matches();
    // Initialize component container.
    let mut container = known_components
        .build_with_arguments(&args)
        .expect("failed to initialize component container");

    let client = contract_client!(signer, evm, args, container);

    let state_path = args.value_of("exported_state").unwrap();
    debug!("Parsing state JSON");
    let state: ExportedState =
        serde_json::from_slice(&filebuffer::FileBuffer::open(state_path).unwrap()).unwrap();
    debug!("Done parsing state JSON");
    debug!("Injecting {} accounts", state.state.len());
    let mut accounts = state.state.into_iter();
    let mut num_accounts_injected = 0;
    loop {
        let chunk = accounts.by_ref().take(INJECT_CHUNK_SIZE);
        let mut accounts_req = Vec::new();
        let mut storage_req = Vec::new();
        for (addr, account) in chunk {
            let address = Address::from_str(strip_0x(&addr)).unwrap();

            let mut account_state = AccountState {
                nonce: U256::from_str(strip_0x(&account.nonce)).unwrap(),
                address: address,
                balance: U256::from_str(strip_0x(&account.balance)).unwrap(),
                code: match account.code {
                    Some(code) => code,
                    None => String::new(),
                },
            };
            if let Some(storage) = account.storage {
                for (key, value) in storage {
                    storage_req.push((
                        address,
                        H256::from_str(strip_0x(&key)).unwrap(),
                        H256::from_str(strip_0x(&value)).unwrap(),
                    ));
                }
            }
            accounts_req.push(account_state);
        }
        if accounts_req.is_empty() && storage_req.is_empty() {
            break;
        }
        let accounts_len = accounts_req.len();
        let res = client.inject_accounts(accounts_req).wait().unwrap();

        debug!("Injecting {} account storage items", storage_req.len());
        for chunk in storage_req.chunks(INJECT_STORAGE_CHUNK_SIZE) {
            let chunk_len = chunk.len();
            let chunk_vec = chunk.to_vec();
            let start = Instant::now();
            let res = client.inject_account_storage(chunk_vec).wait().unwrap();
            let end = Instant::now();
            let duration_ms = to_ms(end - start);
            debug!(
                "Injected {} account storage items in {:.3} ms: {:.3} items/sec",
                chunk_len,
                duration_ms,
                chunk_len as f64 / duration_ms * 1000.
            );
        }

        num_accounts_injected += accounts_len;
        debug!("Injected {} accounts", num_accounts_injected);
    }
    debug!("Done injecting accounts");
    let res = client
        .init_genesis_block(evm_api::InitStateRequest {})
        .wait()
        .unwrap();
}
