extern crate ethabi;
extern crate runtime_ethereum;

mod contracts;

use ethereum_types::{Address, U256};
use runtime_ethereum::test;

use std::io::prelude::*;
use std::fs::File;

#[test]
fn wasi() {
    let mut client = test::Client::new();
    client.gas_limit = 15177522.into();

    let (tx_hash, address) = {
        let mut f = File::open("tests/contracts/mantle-counter/target/service/mantle-counter.wasm")
            .unwrap();

        let mut wasm = Vec::new();
        f.read_to_end(&mut wasm).unwrap();

        let initcode = wasm;

        client.create_contract(initcode, &0.into())
    };

    let result = client.result(tx_hash);

    assert_eq!(result.status_code, 1);
}
