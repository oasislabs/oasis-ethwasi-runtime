extern crate ethcore;
extern crate ethereum_types;
extern crate keccak_hash;
extern crate oasis_ethwasi_runtime;
extern crate oasis_ethwasi_runtime_common;

mod contracts;

use ethcore::vm::ConfidentialCtx;
use ethereum_types::{Address, H256, U256};
use oasis_ethwasi_runtime::test;

/// With a contract of the form
///
/// -------------------------------
///   // CounterNoConstructor.sol
///
///   pragma solidity ^0.4.0;
///
///   contract Counter {
///
///     uint256 _counter;
///
///     function getCounter() public view returns (uint256) {
///       return _counter;
///     }
///
///     function incrementCounter() public {
///       _counter += 1;
///     }
///
///   }
/// ------------------------------
///
/// tests that storage is None and *not* encrypted when the
/// contract is deployed, since no storage is set or modified
/// in the initcode.
///
#[test]
fn deploy_contract_storage_encryption_no_constructor() {
    // Given.
    let mut client = test::Client::new();
    // When.
    let contract = deploy_counter_no_constructor(&mut client);
    // Then.
    let key = client.confidential_storage_key(contract.clone(), H256::from(0));
    let encrypted_storage_counter = client.raw_storage(contract, key);
    assert_eq!(encrypted_storage_counter, None);
}

/// Tests that storage is correctly encrypted after a transaction
/// is made that sets the storage. Uses the contract above, i.e.,
/// `CounterNoConstructor.sol`.
#[test]
fn tx_contract_storage_encryption_no_constructor() {
    // Given.
    let mut client = test::Client::new();
    let contract = deploy_counter_no_constructor(&mut client);
    // When.
    increment_counter(contract.clone(), &mut client);
    // Then.
    let key = client.confidential_storage_key(contract.clone(), H256::from(0));
    let encrypted_storage_counter = client.raw_storage(contract, key).unwrap();
    // Will error if not encrypted.
    let ctx_decrypted_storage_counter = client
        .key_manager_confidential_ctx(contract.clone())
        .decrypt_storage_value(key.to_vec(), encrypted_storage_counter.clone())
        .unwrap();
    // Encrypted storage's length should be expanded from the original value.
    assert_eq!(encrypted_storage_counter.len(), 63);
    // Decryption should be of size H256.
    assert_eq!(ctx_decrypted_storage_counter.len(), 32);
    // Finally ensure the correct value of 1 is stored.
    assert_eq!(
        H256::from(&ctx_decrypted_storage_counter[..32]),
        H256::from(1)
    );

    // Increment again.
    increment_counter(contract.clone(), &mut client);
    let encrypted_storage_counter = client.raw_storage(contract, key).unwrap();
    let ctx_decrypted_storage_counter = client
        .key_manager_confidential_ctx(contract.clone())
        .decrypt_storage_value(key.to_vec(), encrypted_storage_counter.clone())
        .unwrap();
    // Ensure the correct value of 2 is stored.
    assert_eq!(
        H256::from(&ctx_decrypted_storage_counter[..32]),
        H256::from(2)
    );
}

/// With a contract of the form
///
/// -------------------------------
///
///   pragma solidity ^0.4.0;
///
///   contract Counter {
///     uint256 _counter;
///
///     constructor(uint256 counter) {
///       _counter = counter
///     }
///
///     function getCounter() public view returns (uint256) {
///       return _counter;
///     }
///  }
///
/// ------------------------------
///
/// tests that storage is correctly encrypted when storage
/// is set in the initcode.
///
#[test]
fn test_deploy_contract_storage_encryption_with_constructor() {
    // Given.
    let mut client = test::Client::new();
    // When.
    let contract = deploy_counter_with_constructor(&mut client);
    // Then.
    let key = client.confidential_storage_key(contract.clone(), H256::from(0));
    let encrypted_storage_counter = client.raw_storage(contract, key).unwrap();
    // Will error if not encrypted.
    let ctx_decrypted_storage_counter = client
        .key_manager_confidential_ctx(contract.clone())
        .decrypt_storage_value(key.to_vec(), encrypted_storage_counter.clone())
        .unwrap();
    // Encrypted storage's length should be expanded from the original value.
    assert_eq!(encrypted_storage_counter.len(), 63);
    // Decryption should be of size H256.
    assert_eq!(ctx_decrypted_storage_counter.len(), 32);
    // Finally ensure the correct value of 1 is stored.
    assert_eq!(
        H256::from(&ctx_decrypted_storage_counter[..32]),
        H256::from(5)
    );
}

fn deploy_counter_no_constructor<'a>(client: &mut test::Client) -> Address {
    let counter_code = contracts::counter::solidity_initcode();
    let (_, contract) = client.create_confidential_contract(counter_code, &U256::zero());

    // Sanity check.
    let counter_zero = get_counter(&contract, client);
    let expected_zero = [0; 32].to_vec();
    assert_eq!(counter_zero, expected_zero);

    contract
}

/// Makes a *call* to the `getCounter()` method.
fn get_counter<'a>(contract: &Address, client: &mut test::Client) -> Vec<u8> {
    let sighash_data = contracts::counter::get_counter_sighash();
    client.confidential_call(contract, sighash_data, &U256::zero())
}

/// Invokes the `incrementCounter` method on the contract (and does some post
/// validation to sanity check it worked).
fn increment_counter<'a>(contract: Address, client: &mut test::Client) {
    let counter_pre = get_counter(&contract, client);

    let increment_counter_data = contracts::counter::increment_counter_sighash();
    client.confidential_send(Some(&contract), increment_counter_data, &U256::zero());

    // Sanity check.
    let counter_post = get_counter(&contract, client);
    assert_eq!(
        U256::from(&counter_pre[..32]) + U256::from(1),
        U256::from(&counter_post[..32])
    );
}

fn deploy_counter_with_constructor<'a>(client: &mut test::Client) -> Address {
    let initcode =
        hex::decode("608060405234801561001057600080fd5b506040516020806100ea83398101806040528101908080519060200190929190505050806000819055505060a1806100496000396000f300608060405260043610603f576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff1680638ada066e146044575b600080fd5b348015604f57600080fd5b506056606c565b6040518082815260200191505060405180910390f35b600080549050905600a165627a7a72305820137067541d27b3ea9965691d1cb4585098dddc2cc08809233b6a1df18ddc110300290000000000000000000000000000000000000000000000000000000000000005").unwrap();

    let (_, contract) = client.create_confidential_contract(initcode, &U256::zero());

    // Sanity check.
    let counter_five = get_counter(&contract, client);
    let expected_five = vec![
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 5,
    ];
    assert_eq!(counter_five, expected_five);

    contract
}
