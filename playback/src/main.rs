#![feature(use_extern_macros)]
extern crate clap;
use clap::App;
use clap::Arg;
use clap::crate_authors;
use clap::crate_description;
use clap::crate_name;
use clap::crate_version;
use clap::value_t;
extern crate filebuffer;
extern crate futures;
use futures::future::Future;
extern crate grpcio;
extern crate hex;
extern crate rlp;

extern crate client_utils;
use client_utils::contract_client;
use client_utils::default_app;
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

fn main() {
    let seed = ekiden_core::bytes::B256::random();
    let seed_input = ekiden_core::untrusted::Input::from(&seed);
    let key_pair = ekiden_core::ring::signature::Ed25519KeyPair::from_seed_unchecked(seed_input).unwrap();
    let signer = std::sync::Arc::new(ekiden_core::signature::InMemorySigner::new(key_pair));
    let mut client = contract_client!(signer, evm);

    let blocks_path = std::env::args().nth(1).expect("Usage: playback EXPORTED_BLOCKS");
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
            println!("{:?}", res);
        }
    }
}
