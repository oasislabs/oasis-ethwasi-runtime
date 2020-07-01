extern crate ethereum_types;
extern crate oasis_ethwasi_runtime;
extern crate oasis_ethwasi_runtime_api;
extern crate time;

mod contracts;

use ethereum_types::{Address, U256};
use oasis_ethwasi_runtime::test;
use oasis_ethwasi_runtime_api::ExecutionResult;

/// Makes a call to the `getCounter()` method.
fn get_counter<'a>(contract: &Address, client: &mut test::Client) -> U256 {
    let sighash_data = contracts::counter::get_counter_sighash();
    U256::from(
        client
            .call(contract, sighash_data, &U256::zero())
            .as_slice(),
    )
}

/// Invokes the `incrementCounter` method on the contract, returns the receipt.
fn increment_counter<'a>(contract: Address, client: &mut test::Client) -> ExecutionResult {
    let sighash_data = contracts::counter::increment_counter_sighash();
    let (tx_hash, _) = client
        .send(Some(&contract), sighash_data, &U256::zero(), None)
        .expect("incrementing counter should succeed");
    client.result(tx_hash)
}

#[test]
fn test_default_expiry() {
    let mut client = test::Client::new();

    // get current time
    let now = time::get_time().sec as u64;
    client.set_timestamp(now);

    // deploy counter contract without header
    let (_, contract) =
        client.create_contract(contracts::counter::solidity_initcode(), &U256::zero());

    // check that expiry is 100 years in the future
    let expiry = client.storage_expiry(contract);
    assert_eq!(expiry, now + 3155695200);
}

#[test]
fn test_invalid_expiry() {
    let mut client = test::Client::new();

    // get current time
    let now = time::get_time().sec as u64;
    client.set_timestamp(now);

    // attempt to deploy counter contract with invalid expiry
    let deploy_expiry = now - 1;
    let (tx_hash, _) = client.create_contract_with_header(
        contracts::counter::solidity_initcode(),
        &U256::zero(),
        Some(deploy_expiry),
        None,
    );

    // check that deploy failed (0 status code)
    let status = client.result(tx_hash).status_code;
    assert_eq!(status, 0);
}

#[test]
fn test_expiry() {
    let mut client = test::Client::new();

    // get current time
    let deploy_time = time::get_time().sec as u64;
    client.set_timestamp(deploy_time);

    // deploy counter contract with expiry
    let duration = 31557600;
    let (_, contract) = client.create_contract_with_header(
        contracts::counter::solidity_initcode(),
        &U256::zero(),
        Some(deploy_time + duration),
        None,
    );

    // check expiry
    let expiry = client.storage_expiry(contract);
    assert_eq!(expiry, deploy_time + duration);

    // increment counter twice
    let counter_pre = get_counter(&contract, &mut client);
    let _ = increment_counter(contract.clone(), &mut client);
    let receipt_1 = increment_counter(contract.clone(), &mut client);
    let counter_post = get_counter(&contract, &mut client);
    assert_eq!(counter_post, counter_pre + U256::from(2));

    // increment counter (later)
    client.set_timestamp(deploy_time + duration / 2);
    let counter_pre = get_counter(&contract, &mut client);
    let receipt_2 = increment_counter(contract.clone(), &mut client);
    let counter_post = get_counter(&contract, &mut client);
    assert_eq!(counter_post, counter_pre + U256::from(1));

    // check that gas cost is cheaper
    assert!(receipt_2.gas_used < receipt_1.gas_used);

    // increment counter (expired)
    client.set_timestamp(deploy_time + duration + 1);
    let receipt_3 = increment_counter(contract.clone(), &mut client);

    // check that transaction failed (0 status code)
    assert_eq!(receipt_3.status_code, 0);

    let expired_counter = get_counter(&contract, &mut client);
    assert_eq!(expired_counter, U256::from(0));
}
