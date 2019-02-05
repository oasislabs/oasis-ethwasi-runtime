extern crate ethereum_types;
extern crate runtime_ethereum;

mod contracts;

use runtime_ethereum::test;

use ethereum_types::U256;

/// The number of transactions to run for each test. I.e., the number of times we want
/// to check estimate_gas against gas_used for each test and assert they are equal.
const ITERATION_COUNT: u8 = 10;

#[test]
fn estimate_gas_deploy_solidity() {
    estimate_gas_deploy_test(contracts::counter::solidity_initcode(), false);
}

#[test]
fn estimate_gas_deploy_rust() {
    estimate_gas_deploy_test(contracts::counter::rust_initcode(), false);
}

#[test]
fn estimate_gas_tx_solidity() {
    estimate_gas_tx_test(contracts::counter::solidity_initcode(), false);
}

#[test]
fn estimate_gas_tx_rust() {
    estimate_gas_tx_test(contracts::counter::rust_initcode(), false);
}

#[test]
fn estimate_gas_solidity_deploy_confidential() {
    estimate_gas_deploy_test(contracts::counter::solidity_initcode(), true);
}

#[test]
fn estimate_gas_rust_deploy_confidential() {
    estimate_gas_deploy_test(contracts::counter::rust_initcode(), true);
}

#[test]
fn estimate_gas_solidity_tx_confidential() {
    estimate_gas_tx_test(contracts::counter::solidity_initcode(), true);
}

#[test]
fn estimate_gas_rust_tx_confidential() {
    estimate_gas_tx_test(contracts::counter::rust_initcode(), true);
}

/// Regression test for a contract that receives an incorrect estimate gas
/// when it's wasm gas cost is incorrectly scaled.
/// See https://github.com/oasislabs/runtime-ethereum/issues/547
#[test]
fn estimate_gas_wasm_scaling() {
    // Given
    let mut client = test::Client::instance();
    let data = contracts::bulk_storage::initcode();
    // When
    let estimate_gas = client.estimate_gas(None, data.clone(), &U256::from(0));
    let (tx_hash, addr) = client.create_contract(data, &U256::from(0));
    // Then
    let receipt = client.receipt(tx_hash);
    assert_eq!(receipt.cumulative_gas_used, estimate_gas);
}

/// Tests that estimate gas for a deployed transaction is the same as the gas
/// actually used. Runs the test several times to make sure that estimate_gas
/// and gas_used don't change for the same transaction.
fn estimate_gas_deploy_test(initcode: Vec<u8>, confidential: bool) {
    let estimate_gas = estimate_gas_deploy(initcode.clone(), confidential);
    for _ in 0..ITERATION_COUNT {
        let next_estimate_gas = estimate_gas_deploy(initcode.clone(), confidential);
        assert_eq!(estimate_gas, next_estimate_gas);
    }
}

/// Tests that estimate gas for a transaction to an already deployed contract
/// is the same as the gas actually used. Runs the test several times to make
/// sure that estimate_gas and gas_used don't change for the same transaction.
fn estimate_gas_tx_test(initcode: Vec<u8>, confidential: bool) {
    let estimate_gas = estimate_gas_tx(initcode.clone(), confidential);
    for _ in 0..ITERATION_COUNT {
        let next_estimate_gas = estimate_gas_tx(initcode.clone(), confidential);
        assert_eq!(estimate_gas, next_estimate_gas);
    }
}

fn estimate_gas_deploy<'a>(data: Vec<u8>, confidential: bool) -> U256 {
    let mut client = test::Client::instance();

    let (estimate, tx_hash) = if confidential {
        (
            client.confidential_estimate_gas(None, data.clone(), &U256::from(0)),
            client.create_confidential_contract(data, &U256::from(0)).0,
        )
    } else {
        (
            client.estimate_gas(None, data.clone(), &U256::from(0)),
            client.create_contract(data, &U256::from(0)).0,
        )
    };

    let receipt = client.receipt(tx_hash);

    assert_eq!(estimate, receipt.gas_used.unwrap());

    estimate
}

/// Always redploys the contract to make sure the state is the same every time we
/// run this.
fn estimate_gas_tx<'a>(initcode: Vec<u8>, confidential: bool) -> U256 {
    let mut client = test::Client::instance();
    let data = contracts::counter::increment_counter_sighash();

    let (conf_data, address) = if confidential {
        let (_, address) = client.create_confidential_contract(initcode, &U256::from(0));
        (
            client.confidential_data(Some(&address), data.clone()),
            address,
        )
    } else {
        (
            data.clone(),
            client.create_contract(initcode, &U256::from(0)).1,
        )
    };

    let estimate = client.estimate_gas(Some(&address), conf_data.clone(), &U256::from(0));

    let tx_hash = if confidential {
        client.confidential_send(Some(&address), data.clone(), &U256::from(0))
    } else {
        client.send(Some(&address), data.clone(), &U256::from(0))
    };

    let receipt = client.receipt(tx_hash);

    assert_eq!(estimate, receipt.gas_used.unwrap());

    estimate
}
