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

    // deploy counter contract
    let contract = deploy_counter(&mut client);

    // check that expiry is 100 years in the future
    let expiry = client.storage_expiry(contract);
    assert_eq!(expiry, now + 3155695200);
}

fn deploy_counter<'a>(client: &mut MutexGuard<'a, test::Client>) -> Address {
    let code = contracts::counter::solidity_initcode();
    let (_, contract) = client.create_contract(code, &U256::zero());
    contract
}
