extern crate runtime_ethereum;

use ethereum_types::{Address, U256};
use runtime_ethereum::test;

use std::{fs::File, io::prelude::*};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct RpcPayload {
    method: String,
    payload: Vec<serde_cbor::Value>,
}

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
    let get_count_data = serde_cbor::to_vec(&RpcPayload {
        method: "get_count".to_string(),
        payload: Vec::new(),
    })
    .unwrap();

    // get_count
    {
        let output = client.call(&address, get_count_data.clone(), &0.into());
        assert_eq!(output, vec![0]);
    }

    // increment_count
    {
        // Generated: new Uint8Array(cbor.encode({ method: 'increment_count',  payload: {} }))
        let increment_count_data = serde_cbor::to_vec(&RpcPayload {
            method: "increment_count".to_string(),
            payload: Vec::new(),
        })
        .unwrap();
        dbg!("SENDING ICREMENT COUNT");
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
