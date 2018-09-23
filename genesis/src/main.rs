use std::collections::HashMap;
use std::str::FromStr;

extern crate clap;
use clap::{crate_authors, crate_description, crate_name, crate_version, value_t_or_exit, App, Arg};
extern crate ethereum_types;
use ethereum_types::{Address, H256, U256};
extern crate filebuffer;
extern crate futures;
use futures::future::Future;
extern crate hex;
extern crate log;
use log::debug;
extern crate pretty_env_logger;
extern crate rlp;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
use serde_json::{de::SliceRead, StreamDeserializer};

extern crate client_utils;
use client_utils::{contract_client, default_app};
extern crate ekiden_contract_client;
use ekiden_contract_client::create_contract_client;
extern crate ekiden_core;
extern crate ekiden_rpc_client;
extern crate ekiden_tracing;

extern crate ethereum_api;
use ethereum_api::{with_api, AccountState};

use std::time::{Duration, Instant};

with_api! {
    create_contract_client!(ethereum, ethereum_api, api);
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

fn to_ms(d: Duration) -> f64 {
    d.as_secs() as f64 * 1e3 + d.subsec_nanos() as f64 * 1e-6
}

fn strip_0x(hex: &str) -> &str {
    if hex.starts_with("0x") {
        hex.get(2..).unwrap()
    } else {
        hex
    }
}

const EXPORTED_STATE_START: &[u8] = b"{ \"state\": {";
const EXPORTED_STATE_ACCOUNT_SEP: &[u8] = b",";
const EXPORTED_STATE_ADDR_SEP: &[u8] = b": ";
const EXPORTED_STATE_END: &[u8] = b"\n}}";

enum StateParsingState {
    /// { "state": {
    ///             ^
    First,
    /// "0x...": {...}
    ///               ^
    Middle,
    /// }}
    ///   ^
    End,
}

/// Streaming parser for Parity's exported state JSON.
/// https://github.com/paritytech/parity-ethereum/blob/v1.9.7/parity/blockchain.rs#L633-L689
struct StateParser<'a> {
    src: &'a [u8],
    state: StateParsingState,
}

impl<'a> StateParser<'a> {
    fn new(src: &'a [u8]) -> Self {
        let (start, rest) = src.split_at(EXPORTED_STATE_START.len());
        assert_eq!(start, EXPORTED_STATE_START);
        Self {
            src: rest,
            state: StateParsingState::First,
        }
    }
}

impl<'a> Iterator for StateParser<'a> {
    type Item = (String, ExportedAccount);

    fn next(&mut self) -> Option<(String, ExportedAccount)> {
        // }}
        //   ^
        if let StateParsingState::End = self.state {
            return None;
        }

        // \n}}
        // --->^
        let (end, rest) = self.src.split_at(EXPORTED_STATE_END.len());
        if end == EXPORTED_STATE_END {
            self.src = rest;
            self.state = StateParsingState::End;
            return None;
        }

        // ...,
        //    >^
        if let StateParsingState::Middle = self.state {
            let (account_sep, rest) = self.src.split_at(EXPORTED_STATE_ACCOUNT_SEP.len());
            assert_eq!(account_sep, EXPORTED_STATE_ACCOUNT_SEP);
            self.src = rest;
        }

        // \n"0x...": {...}
        // -------->^
        let mut de_addr = StreamDeserializer::new(SliceRead::new(self.src));
        let addr = de_addr.next().unwrap().unwrap();
        let (_, rest) = self.src.split_at(de_addr.byte_offset());
        self.src = rest;

        // "0x...": {...}
        //        ->^
        let (addr_sep, rest) = self.src.split_at(EXPORTED_STATE_ADDR_SEP.len());
        assert_eq!(addr_sep, EXPORTED_STATE_ADDR_SEP);
        self.src = rest;

        // "0x...": {...}
        //          ---->^
        let mut de_account = StreamDeserializer::new(SliceRead::new(self.src));
        let account = de_account.next().unwrap().unwrap();
        let (_, rest) = self.src.split_at(de_account.byte_offset());
        self.src = rest;

        self.state = StateParsingState::Middle;
        Some((addr, account))
    }
}

fn main() {
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

    // Initialize tracing.
    ekiden_tracing::report_forever("genesis", &args);

    let client = contract_client!(ethereum, args, container);

    let state_path = args.value_of("exported_state").unwrap();
    let state_fb = filebuffer::FileBuffer::open(state_path).unwrap();
    let mut accounts = StateParser::new(&state_fb);
    let mut num_accounts_injected = 0;
    loop {
        let chunk = accounts.by_ref().take(INJECT_CHUNK_SIZE);
        let mut accounts_req = Vec::new();
        let mut storage_req = Vec::new();
        for (addr, account) in chunk {
            let address = Address::from_str(strip_0x(&addr)).unwrap();

            let mut account_state = AccountState {
                nonce: U256::from_str(strip_0x(&account.nonce)).unwrap(),
                address,
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
        debug!("Injecting {} accounts", accounts_len);
        client.inject_accounts(accounts_req).wait().unwrap();

        debug!("Injecting {} account storage items", storage_req.len());
        for chunk in storage_req.chunks(INJECT_STORAGE_CHUNK_SIZE) {
            let chunk_len = chunk.len();
            let chunk_vec = chunk.to_vec();
            let start = Instant::now();
            client.inject_account_storage(chunk_vec).wait().unwrap();
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
        debug!(
            "Injected {} accounts, {} total",
            accounts_len, num_accounts_injected
        );
    }
    debug!("Done injecting accounts");
}
