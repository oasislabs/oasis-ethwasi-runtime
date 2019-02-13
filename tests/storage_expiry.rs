extern crate ethereum_types;
extern crate runtime_ethereum;
extern crate time;

mod contracts;

use ethereum_types::{Address, U256};
use runtime_ethereum::test;
use std::sync::MutexGuard;

/// Makes a call to the `getCounter()` method.
fn get_counter<'a>(contract: &Address, client: &mut MutexGuard<'a, test::Client>) -> U256 {
    let sighash_data = contracts::counter::get_counter_sighash();
    U256::from(
        client
            .call(contract, sighash_data, &U256::zero())
            .as_slice(),
    )
}

/// Invokes the `incrementCounter` method on the contract, returns the status code.
fn increment_counter<'a>(contract: Address, client: &mut MutexGuard<'a, test::Client>) -> u64 {
    let sighash_data = contracts::counter::increment_counter_sighash();
    let tx_hash = client.send(Some(&contract), sighash_data, &U256::zero());
    client.receipt(tx_hash).status_code.unwrap()
}

#[test]
fn test_default_expiry() {
    // get current time
    let now = time::get_time().sec as u64;

    let mut client = test::Client::instance();
    client.set_timestamp(now);

    // deploy counter contract without header
    let (_, contract) =
        client.create_contract(contracts::counter::solidity_initcode(), &U256::zero());

    // check that expiry is 100 years in the future
    let expiry = client.storage_expiry(contract);
    assert_eq!(expiry, now + 3155695200);
}

#[test]
fn test_expiry() {
    // get current time
    let now = time::get_time().sec as u64;

    let mut client = test::Client::instance();
    client.set_timestamp(now);

    // deploy counter contract with expiry
    let deploy_expiry = now + 3600;
    let (_, contract) = client.create_contract_with_header(
        contracts::counter::solidity_initcode(),
        &U256::zero(),
        Some(deploy_expiry),
        None,
    );

    // check expiry
    let expiry = client.storage_expiry(contract);
    assert_eq!(expiry, deploy_expiry);

    // increment counter (not expired)
    let counter_pre = get_counter(&contract, &mut client);
    increment_counter(contract.clone(), &mut client);
    let counter_post = get_counter(&contract, &mut client);
    assert_eq!(counter_post, counter_pre + U256::from(1));

    // increment counter (expired)
    client.set_timestamp(deploy_expiry + 1);
    let status = increment_counter(contract.clone(), &mut client);

    // check that transaction failed (0 status code)
    assert_eq!(status, 0);

    let expired_counter = get_counter(&contract, &mut client);
    assert_eq!(expired_counter, U256::from(0));
}
