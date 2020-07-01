extern crate ethabi;
extern crate oasis_ethwasi_runtime;

mod contracts;

use ethereum_types::{Address, U256};
use oasis_ethwasi_runtime::test;

/// Deploys a confidential contract from within a confidential
/// contract and sets storage on that deploy.
#[test]
fn create_and_set_storage() {
    let mut client = test::Client::new();

    // Factory address.
    let factory_address = {
        let initcode = ethabi::Constructor { inputs: vec![] }
            .encode_input(contracts::factory::bytecode(), &[])
            .unwrap();
        client.create_contract(initcode, &0.into()).1
    };

    // Address of the contract deployed *through* the factory.
    let deployed_addr = {
        let deploy_contract_data = ethabi::Function {
            name: "deployContract".to_string(),
            inputs: vec![
                ethabi::Param {
                    name: "_a".to_string(),
                    kind: ethabi::ParamType::Uint(256),
                },
                ethabi::Param {
                    name: "_c".to_string(),
                    kind: ethabi::ParamType::Uint(256),
                },
                ethabi::Param {
                    name: "_b".to_string(),
                    kind: ethabi::ParamType::Array(Box::new(ethabi::ParamType::Uint(256))),
                },
            ],
            outputs: vec![],
            constant: true,
        }
        .encode_input(&[
            ethabi::Token::Uint(33.into()),
            ethabi::Token::Uint(55.into()),
            ethabi::Token::Array(vec![
                ethabi::Token::Uint(1.into()),
                ethabi::Token::Uint(2.into()),
                ethabi::Token::Uint(3.into()),
            ]),
        ])
        .unwrap();

        let (tx_hash, _) = client
            .send(
                Some(&factory_address),
                deploy_contract_data,
                &0.into(),
                None,
            )
            .expect("deployment should succeed");

        // Ensure it didn't revert.
        let receipt = client.result(tx_hash);
        assert_eq!(receipt.status_code, 1);
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&receipt.logs[0].data[12..]);
        Address::from(addr)
    };

    // Return value of retrieveA().
    let retrieve_a = {
        let retrieve_a_data = ethabi::Function {
            name: "retrieveA".to_string(),
            inputs: vec![],
            outputs: vec![ethabi::Param {
                name: "".to_string(),
                kind: ethabi::ParamType::Uint(256),
            }],
            constant: true,
        }
        .encode_input(&[])
        .unwrap();

        let result = client.call(&deployed_addr, retrieve_a_data, &0.into());
        U256::from(result.as_slice())
    };

    // Return value of retrieveB().
    let retrieve_b = {
        let retrieve_a_data = ethabi::Function {
            name: "retrieveB".to_string(),
            inputs: vec![],
            outputs: vec![ethabi::Param {
                name: "".to_string(),
                kind: ethabi::ParamType::Uint(256),
            }],
            constant: true,
        }
        .encode_input(&[])
        .unwrap();

        let result = client.call(&deployed_addr, retrieve_a_data, &0.into());
        U256::from(result.as_slice())
    };

    // Return value of retrieveC().
    let retrieve_c = {
        let retrieve_a_data = ethabi::Function {
            name: "retrieveC".to_string(),
            inputs: vec![],
            outputs: vec![ethabi::Param {
                name: "".to_string(),
                kind: ethabi::ParamType::Uint(256),
            }],
            constant: true,
        }
        .encode_input(&[])
        .unwrap();

        let result = client.call(&deployed_addr, retrieve_a_data, &0.into());
        (&ethabi::decode(
            &[ethabi::ParamType::Array(Box::new(ethabi::ParamType::Uint(
                256,
            )))],
            &result,
        )
        .unwrap()[0])
            .clone()
            .to_array()
            .unwrap()
    };

    // Now check all the retrieved data.
    assert_eq!(
        vec![
            ethabi::Token::Uint(1.into()),
            ethabi::Token::Uint(2.into()),
            ethabi::Token::Uint(3.into())
        ],
        retrieve_c,
    );
    assert_eq!(retrieve_a, U256::from(33));
    assert_eq!(retrieve_b, U256::from(55));
}
