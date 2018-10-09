#[macro_use]
extern crate clap;
extern crate either;
extern crate ethcore;
extern crate log;
extern crate run_contract;
extern crate simple_logger;

use std::fs;

use clap::Arg;
use either::Either;
use ethcore::rlp;

use run_contract::{make_tx, run_tx, store_bytes};

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
        .arg(
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
        .arg(
            Arg::with_name("call-args")
                .long("call-args")
                .help("Arguments to be passed to contract call")
                .takes_value(true)
                .default_value(""),
        )
        .get_matches();

    match args.occurrences_of("v") {
        1 => simple_logger::init_with_level(log::Level::Trace),
        2 => simple_logger::init_with_level(log::Level::Debug),
        3 => simple_logger::init_with_level(log::Level::Info),
        4 => simple_logger::init_with_level(log::Level::Warn),
        5 => simple_logger::init_with_level(log::Level::Error),
        _ => Ok(()),
    }.expect("initialize simple logger");

    println!("{:?}", store_bytes(&[1, 2, 3, 4, 5]));
    let contract = fs::read(args.value_of("contract").unwrap()).unwrap();
    let create_tx = make_tx(Either::Left(contract));
    if let Some(tx_file) = args.value_of("dump-tx") {
        fs::write(tx_file, rlp::encode(&create_tx)).unwrap();
    }
    let contract_address = run_tx(create_tx).unwrap().contract_address.unwrap();
    let call_args = args.value_of("call-args").unwrap().as_bytes().to_owned();
    println!(
        "{:?}",
        run_tx(make_tx(Either::Right((contract_address, call_args)))).unwrap()
    )
}
