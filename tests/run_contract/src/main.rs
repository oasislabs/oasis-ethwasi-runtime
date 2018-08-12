#[macro_use]
extern crate clap;
extern crate either;
extern crate ethcore;
extern crate ethereum_api;
extern crate ethereum_types;
extern crate ethkey;
#[macro_use]
extern crate lazy_static;
extern crate runtime_ethereum;

use clap::Arg;
use either::Either;
use ethcore::{rlp,
              transaction::{Action, SignedTransaction, Transaction}};
use ethereum_api::Receipt;
use ethereum_types::{Address, U256};
use ethkey::Secret;
use runtime_ethereum::{execute_raw_transaction, get_account_nonce, get_receipt};
use std::{fs, str::FromStr};

lazy_static! {
    static ref DEFAULT_ACCOUNT: Address = Address::from("1cca28600d7491365520b31b466f88647b9839ec");
    static ref SECRET_KEY: Secret = Secret::from_str(
        // private key corresponding to DEFAULT_ACCOUNT. generated from mnemonic:
        // patient oppose cotton portion chair gentle jelly dice supply salmon blast priority
        "c61675c22aee77da8f6e19444ece45557dc80e1482aa848f541e94e3e5d91179"
    ).unwrap();
}

fn make_tx(spec: Either<Vec<u8>, Address>) -> SignedTransaction {
    let mut tx = Transaction::default();
    tx.gas = U256::from("10000000000000");
    tx.nonce = U256::from(get_account_nonce(&DEFAULT_ACCOUNT).unwrap());
    match spec {
        Either::Left(data) => tx.data = data,
        Either::Right(addr) => tx.action = Action::Call(addr),
    };
    tx.sign(&SECRET_KEY, None)
}

fn run(tx: SignedTransaction) -> Receipt {
    let res = execute_raw_transaction(&rlp::encode(&tx).to_vec()).unwrap();
    let receipt = get_receipt(res.hash.as_ref().unwrap()).unwrap().unwrap();
    if !receipt.status_code.is_some() || receipt.status_code.unwrap() == 0 {
        panic!("{:?}", &res);
    }
    receipt
}

fn main() {
    let args = app_from_crate!()
        .arg(
            Arg::with_name("contract")
                .help("path to file containing contract bytecode")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("dump-tx")
                .long("dump-tx")
                .value_name("FILE")
                .help("dump RLP-encoded transaction to file")
                .takes_value(true),
        )
        .get_matches();

    let contract = fs::read(args.value_of("contract").unwrap()).unwrap();
    let create_tx = make_tx(Either::Left(contract));
    if let Some(tx_file) = args.value_of("dump-tx") {
        fs::write(tx_file, rlp::encode(&create_tx)).unwrap();
    }
    println!("\nDeploying contract...\n=====================");
    let create_receipt = run(create_tx);
    let contract_addr = create_receipt.contract_address.unwrap();
    println!("\nCalling contract...\n=====================");
    println!("{:?}", run(make_tx(Either::Right(contract_addr))))
}
