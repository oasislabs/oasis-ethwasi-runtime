extern crate ethereum_types;
extern crate runtime_ethereum;
extern crate time;

mod contracts;

use ethereum_types::{Address, U256};
use runtime_ethereum::test;
use std::sync::MutexGuard;

#[test]
fn test_default_expiry() {
    // get current time
    let now = time::get_time().sec as u64;

    let mut client = test::Client::instance();
    client.set_timestamp(now);

    // deploy counter contract without header
    let code = contracts::counter::solidity_initcode();
    let (_, contract) = client.create_contract(code, &U256::zero());

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
    let code = contracts::counter::solidity_initcode();
    let (_, contract) =
        client.create_contract_with_header(code, &U256::zero(), Some(deploy_expiry), None);

    // check expiry
    let expiry = client.storage_expiry(contract);
    assert_eq!(expiry, deploy_expiry);
}
