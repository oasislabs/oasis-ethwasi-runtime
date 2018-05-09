#![feature(use_extern_macros)]
extern crate clap;
use clap::App;
use clap::Arg;
use clap::value_t;
extern crate filebuffer;
extern crate futures;
use futures::future::Future;
extern crate grpcio;
extern crate hex;
extern crate rlp;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

extern crate client_utils;
use client_utils::contract_client;
use client_utils::default_backend;
extern crate ekiden_contract_client;
use ekiden_contract_client::create_contract_client;
extern crate ekiden_core;
extern crate ekiden_rpc_client;

extern crate evm_api;
use evm_api::with_api;

with_api! {
    create_contract_client!(evm, evm_api, api);
}

#[derive(Deserialize)]
struct ExportedAccount {
    balance: String,
    nonce: String,
}
#[derive(Deserialize)]
struct ExportedState {
    state: std::collections::HashMap<String, ExportedAccount>,
}

fn main() {
    let seed = ekiden_core::bytes::B256::random();
    let seed_input = ekiden_core::untrusted::Input::from(&seed);
    let key_pair = ekiden_core::ring::signature::Ed25519KeyPair::from_seed_unchecked(seed_input).unwrap();
    let signer = std::sync::Arc::new(ekiden_core::signature::InMemorySigner::new(key_pair));
    let args = App::new("playback client")
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
        .arg(Arg::with_name("host")
             .long("host")
             .short("h")
             .takes_value(true)
             .default_value("127.0.0.1")
             .display_order(1))
        .arg(Arg::with_name("port")
             .long("port")
             .short("p")
             .takes_value(true)
             .default_value("9001")
             .display_order(2))
        .arg(Arg::with_name("nodes")
             .long("nodes")
             .help("A list of comma-separated compute node addresses (e.g. host1:9001,host2:9004)")
             .takes_value(true))
        .arg(Arg::with_name("mr-enclave")
             .long("mr-enclave")
             .value_name("MRENCLAVE")
             .help("MRENCLAVE in hex format")
             .takes_value(true)
             .required(true)
             .display_order(3))
        .get_matches();
    let mut client = contract_client!(signer, evm, args);

    let state_path = args.value_of("exported_state").unwrap();
    let state: ExportedState = serde_json::from_reader(std::fs::File::open(state_path).unwrap()).unwrap();
    let res = client.init_genesis_block({
        let mut req = evm_api::InitStateRequest::new();
        for (addr, account) in state.state {
            let mut account_state = evm_api::AccountState::new();
            account_state.set_nonce(account.nonce);
            account_state.set_address(addr);
            account_state.set_balance(account.balance);
            req.accounts.push(account_state);
        }
        req
    }).wait().unwrap();
    println!("init_genesis_block: {:?}", res);

    let blocks_path = args.value_of("exported_blocks").unwrap();
    // Blocks are written one after another into the exported blocks file.
    // https://github.com/paritytech/parity/blob/v1.9.7/parity/blockchain.rs#L595
    let blocks_raw = filebuffer::FileBuffer::open(blocks_path).unwrap();
    let mut offset = 0;
    while offset < blocks_raw.len() {
        // Each block is a 3-list of (header, transactions, uncles).
        // https://github.com/paritytech/parity/blob/v1.9.7/ethcore/src/encoded.rs#L188
        let start = offset;
        let payload_info = rlp::PayloadInfo::from(&blocks_raw[start..]).unwrap();
        let end = start + payload_info.total();
        let block = rlp::Rlp::new(&blocks_raw[start..end]);
        offset = end;
        // https://github.com/paritytech/parity/blob/v1.9.7/ethcore/src/views/block.rs#L101
        let transactions = block.at(1);
        for transaction in transactions.iter() {
            let transaction_raw = transaction.as_raw();
            let res = client.execute_raw_transaction({
                let mut req = evm_api::ExecuteRawTransactionRequest::new();
                req.set_data(hex::encode(transaction_raw));
                req
            }).wait().unwrap();
            println!("execute_raw_transaction: {:?}", res);
        }
    }
}
