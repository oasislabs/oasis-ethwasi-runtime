#[macro_use]
extern crate clap;
extern crate either;
extern crate ethcore;
extern crate run_contract;

use std::fs;

use clap::Arg;
use either::Either;
use ethcore::rlp;

use run_contract::{make_tx, run_tx};

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
    let contract_address = run_tx(create_tx).unwrap().contract_address.unwrap();
    println!(
        "{:?}",
        run_tx(make_tx(Either::Right(contract_address))).unwrap()
    )
}
