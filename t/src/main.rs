extern crate runtime_ethereum;

use ethereum_types::{Address, U256};
use runtime_ethereum::test;

use std::io::prelude::*;
use std::fs::File;


fn main() {
    let mut client = test::Client::new();
    client.gas_limit = 15177522.into();

    // Deploy mantle contract.
    let address = {
        let mut f = File::open("/code/runtime-ethereum/tests/contracts/mantle-counter/target/service/mantle-counter.wasm")
            .unwrap();

        let mut wasm = Vec::new();
        f.read_to_end(&mut wasm).unwrap();

        let initcode = wasm;

        let (tx_hash, address) = client.create_contract(initcode, &0.into());
        let result = client.result(tx_hash);
        assert_eq!(result.status_code, 1);

        address
    };

    // Generated: new Uint8Array(cbor.encode({ method: 'get_count',  payload: {} }))
    let get_count_data = vec![162, 102, 109, 101, 116, 104, 111, 100, 105, 103, 101, 116, 95, 99, 111, 117, 110, 116, 103, 112, 97, 121, 108, 111, 97, 100, 160];

    // get_count
    {
        let output = client.call(&address, get_count_data.clone(), &0.into());
        assert_eq!(output, vec![0]);
    }

    // increment_count
    {
        // Generated: new Uint8Array(cbor.encode({ method: 'increment_count',  payload: {} }))
        let increment_count_data = vec![162, 102, 109, 101, 116, 104, 111, 100, 111, 105, 110, 99, 114, 101, 109, 101, 110, 116, 95, 99, 111, 117, 110, 116, 103, 112, 97, 121, 108, 111, 97, 100, 160];
        let (hash, _) = client.send(Some(&address), increment_count_data, &0.into());

        let result = client.result(hash);
        assert_eq!(result.status_code, 1);
    }

    // get_count
    {
        let output = client.call(&address, get_count_data.clone(), &0.into());
        assert_eq!(output, vec![1]);
    }
}
