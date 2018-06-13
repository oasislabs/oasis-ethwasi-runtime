#![feature(use_extern_macros)]
use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;
use std::time::Duration;
use std::time::Instant;

extern crate ethereum_types;
extern crate clap;
use clap::crate_authors;
use clap::crate_description;
use clap::crate_name;
use clap::crate_version;
use clap::value_t_or_exit;
use clap::App;
use clap::Arg;
extern crate filebuffer;
extern crate futures;
use futures::future::Future;
extern crate grpcio;
extern crate hex;
extern crate log;
use log::debug;
use log::info;
use log::log;
use log::trace;
extern crate pretty_env_logger;
extern crate rlp;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

extern crate client_utils;
use client_utils::contract_client;
use client_utils::default_app;
extern crate ekiden_contract_client;
use ekiden_contract_client::create_contract_client;
extern crate ekiden_core;
use ekiden_core::bytes::B256;
extern crate ekiden_rpc_client;

extern crate evm_api;
use evm_api::{with_api, AccountState};

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
        .arg(
            Arg::with_name("exported_blocks")
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
    trace!("Parsing state JSON");
    let state: ExportedState =
        serde_json::from_slice(&filebuffer::FileBuffer::open(state_path).unwrap()).unwrap();
    trace!("Done parsing state JSON");
    trace!("Injecting {} accounts", state.state.len());
    let mut accounts = state.state.into_iter();
    let mut num_accounts_injected = 0;
    loop {
        let chunk = accounts.by_ref().take(INJECT_CHUNK_SIZE);
        let mut accounts_req = Vec::new();
        let mut storage_req = Vec::new();
        for (addr, account) in chunk {
            let address = ethereum_types::Address::from_str(&addr).unwrap();

            let mut account_state = AccountState {
                nonce: ethereum_types::U256::from_str(&account.nonce).unwrap(),
                address: address,
                balance: ethereum_types::U256::from_str(&account.balance).unwrap(),
                code: match account.code {
                    Some(code) => code,
                    None => String::new(),
                },
            };
            if let Some(storage) = account.storage {
                for (key, value) in storage {
                    storage_req.push((
                        address,
                        ethereum_types::U256::from_str(&key).unwrap(),
                        ethereum_types::M256::from_str(&value).unwrap(),
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
        debug!("inject_accounts result: {:?}", res); // %%%

        trace!("Injecting {} account storage items", storage_req.len());
        for chunk in storage_req.chunks(INJECT_STORAGE_CHUNK_SIZE) {
            let chunk_len = chunk.len();
            let res = client.inject_account_storage(chunk.to_vec()).wait().unwrap();
            debug!("inject_account_storage result: {:?}", res); // %%%
            trace!("Injected {} account storage items", chunk_len);
        }

        num_accounts_injected += accounts_len;
        trace!("Injected {} accounts", num_accounts_injected);
    }
    trace!("Done injecting accounts");
    let res = client
        .init_genesis_block(evm_api::InitStateRequest {})
        .wait()
        .unwrap();
    debug!("init_genesis_block result: {:?}", res);

    let blocks_path = args.value_of("exported_blocks").unwrap();
    // Blocks are written one after another into the exported blocks file.
    // https://github.com/paritytech/parity/blob/v1.9.7/parity/blockchain.rs#L595
    let blocks_raw = filebuffer::FileBuffer::open(blocks_path).unwrap();
    let mut offset = 0;
    let mut num_transactions = 0;
    let mut transaction_durs = vec![];
    let playback_start = Instant::now();
    while offset < blocks_raw.len() {
        // Each block is a 3-list of (header, transactions, uncles).
        // https://github.com/paritytech/parity/blob/v1.9.7/ethcore/src/encoded.rs#L188
        let start = offset;
        let payload_info = rlp::PayloadInfo::from(&blocks_raw[start..]).unwrap();
        let end = start + payload_info.total();
        let block = rlp::Rlp::new(&blocks_raw[start..end]);
        offset = end;
        trace!("Processing block at offset {}", start);
        // https://github.com/paritytech/parity/blob/v1.9.7/ethcore/src/views/block.rs#L101
        let transactions = block.at(1);
        for transaction in transactions.iter() {
            let transaction_raw = transaction.as_raw();
            let transaction_start = Instant::now();
            let res = client
                .execute_raw_transaction({ hex::encode(transaction_raw) })
                .wait()
                .unwrap();
            let transaction_end = Instant::now();
            let transaction_dur = transaction_end - transaction_start;
            debug!("execute_raw_transaction result: {:?}", res);
            num_transactions += 1;
            transaction_durs.push(transaction_dur);
        }
    }
    let playback_end = Instant::now();
    let playback_dur = playback_end - playback_start;
    info!(
        "Played back {} transactions over {:.3} ms",
        num_transactions,
        to_ms(playback_dur)
    );
    if num_transactions > 0 {
        info!(
            "Throughput: {:.3} ms/tx",
            to_ms(playback_dur) / num_transactions as f64
        );
        info!(
            "Throughput: {:.3} tx/sec",
            num_transactions as f64 / to_ms(playback_dur) * 1000.
        );

        transaction_durs.sort();
        info!(
            "Latency: min {:.3} ms",
            to_ms(*transaction_durs.first().unwrap())
        );
        for pct in [1, 10, 50, 90, 99].iter() {
            let index = std::cmp::min(
                num_transactions - 1,
                (*pct as f64 / 100. * transaction_durs.len() as f64).ceil() as usize,
            );
            info!(
                "Latency: {:2}% {:?} ms",
                pct,
                to_ms(transaction_durs[index])
            );
        }
        info!(
            "Latency: max {:?} ms",
            to_ms(*transaction_durs.last().unwrap())
        );
    }
}
