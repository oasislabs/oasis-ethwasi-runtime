extern crate ethcore;
extern crate ethereum_types;
extern crate keccak_hash;
extern crate runtime_ethereum;
extern crate runtime_ethereum_common;

use ethcore::state::ConfidentialCtx;
use ethereum_types::{Address, H256, U256};
use runtime_ethereum::test;
use std::sync::MutexGuard;

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
fn test_deploy_contract_storage_encryption_no_constructor() {
    // Given.
    let mut client = test::Client::instance();
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
fn test_tx_contract_storage_encryption_no_constructor() {
    // Given.
    let mut client = test::Client::instance();
    let contract = deploy_counter_no_constructor(&mut client);
    // When.
    increment_counter(contract.clone(), &mut client);
    // Then.
    let key = client.confidential_storage_key(contract.clone(), H256::from(0));
    let encrypted_storage_counter = client.raw_storage(contract, key).unwrap();
    // Will error if not encrypted.
    let ctx_decrypted_storage_counter = client
        .key_manager_confidential_ctx(contract.clone())
        .decrypt_storage(encrypted_storage_counter.clone())
        .unwrap();
    // Encrypted storage's length should be expanded from the original value.
    assert_eq!(encrypted_storage_counter.len(), 48);
    // Decryption should be of size H256.
    assert_eq!(ctx_decrypted_storage_counter.len(), 32);
    // Finally ensure the correct value of 1 is stored.
    assert_eq!(
        H256::from_slice(&ctx_decrypted_storage_counter[..32]),
        H256::from(1)
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
    let mut client = test::Client::instance();
    // When.
    let contract = deploy_counter_with_constructor(&mut client);
    // Then.
    let key = client.confidential_storage_key(contract.clone(), H256::from(0));
    let encrypted_storage_counter = client.raw_storage(contract, key).unwrap();
    // Will error if not encrypted.
    let ctx_decrypted_storage_counter = client
        .key_manager_confidential_ctx(contract.clone())
        .decrypt_storage(encrypted_storage_counter.clone())
        .unwrap();
    // Encrypted storage's length should be expanded from the original value.
    assert_eq!(encrypted_storage_counter.len(), 48);
    // Decryption should be of size H256.
    assert_eq!(ctx_decrypted_storage_counter.len(), 32);
    // Finally ensure the correct value of 1 is stored.
    assert_eq!(
        H256::from_slice(&ctx_decrypted_storage_counter[..32]),
        H256::from(5)
    );
}

/// With a contract of the form
///
/// -------------------------------
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
fn deploy_counter_no_constructor<'a>(client: &mut MutexGuard<'a, test::Client>) -> Address {
    let counter_code =
        hex::decode("608060405234801561001057600080fd5b506101b3806100206000396000f300608060405260043610610057576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff1680635b34b9661461005c5780637531dafc146100735780638ada066e146100a0575b600080fd5b34801561006857600080fd5b506100716100cb565b005b34801561007f57600080fd5b5061009e60048036038101908080359060200190929190505050610116565b005b3480156100ac57600080fd5b506100b561017e565b6040518082815260200191505060405180910390f35b600160008082825401925050819055507f20d8a6f5a693f9d1d627a598e8820f7a55ee74c183aa8f1a30e8d4e8dd9a8d846000546040518082815260200191505060405180910390a1565b60008090505b8181101561017a57600160008082825401925050819055507f20d8a6f5a693f9d1d627a598e8820f7a55ee74c183aa8f1a30e8d4e8dd9a8d846000546040518082815260200191505060405180910390a1808060010191505061011c565b5050565b600080549050905600a165627a7a7230582014739e9b3a5a1416c38c575513b4825a6bcf30121c099700b1f2988752659d3b0029").unwrap();

    let (_, contract) = client.create_confidential_contract(counter_code, &U256::zero());

    // Sanity check.
    let counter_zero = get_counter(&contract, client);
    let expected_zero = [0; 32].to_vec();
    assert_eq!(counter_zero, expected_zero);

    contract
}

/// Makes a *call* to the `getCounter()` method.
fn get_counter<'a>(contract: &Address, client: &mut MutexGuard<'a, test::Client>) -> Vec<u8> {
    // Standard ethereum sighash of the getCounter method, i.e. keccak(getCounter()).
    let sighash_data = hex::decode("8ada066e").unwrap();
    client.confidential_call(contract, sighash_data, &U256::zero())
}

/// Invokes the `incrementCounter` method on the contract (and does some post
/// validation to sanity check it worked).
fn increment_counter<'a>(contract: Address, client: &mut MutexGuard<'a, test::Client>) {
    let increment_counter_data = hex::decode("5b34b966").unwrap();
    client.confidential_send(Some(&contract), increment_counter_data, &U256::zero());

    // Sanity check.
    let counter_one = get_counter(&contract, client);
    let expected_one = vec![
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 1,
    ];
    assert_eq!(counter_one, expected_one);
}

fn deploy_counter_with_constructor<'a>(client: &mut MutexGuard<'a, test::Client>) -> Address {
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
