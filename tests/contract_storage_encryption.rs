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
/// this tests that storage is correctly encrypted after a transaction
/// is made that sets the storage. It does this by
///
/// - deploying the contract
/// - validating storage is None
/// - incrementing the counter via transaction
/// - validating the encrypted storage.
///
#[test]
fn test_contract_storage_encryption() {
    let mut client = test::Client::instance();
    // First, create the contract.
    let contract = deploy_counter_contract(&mut client);
    // Second, peak under the hood and access the state directly. Check the value associated
    // with the contract's `_counter` variable is correct, i.e., None, because it has not been
    // altered or set in any way, yet, since we've only run the constructor.
    validate_counter_storage_is_none(contract.clone(), &mut client);
    // Third, execute a transaction that invokes the "increment" method, setting the contract's
    // `_counter` to be 1. Assert this works by issuing a subsequent call to `getCounter`, which
    // should give 1.
    increment_counter(contract.clone(), &mut client);
    // Fourth, now, we have a confidential contract, and we have ensured that contract
    // has written encrypted state to storage. Let's Make sure the storage is actually encrypted
    // by peering directly into the state, pulling out the encrypted storage value, and manually
    // decrypting it.
    validate_counter_storage_is_encrypted(contract, &mut client, H256::from(1));
}

/// Deploys the Counter contract (and does some post validation to sanity check the deploy).
fn deploy_counter_contract<'a>(client: &mut MutexGuard<'a, test::Client>) -> Address {
    let counter_code =
        hex::decode("608060405234801561001057600080fd5b506101b3806100206000396000f300608060405260043610610057576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff1680635b34b9661461005c5780637531dafc146100735780638ada066e146100a0575b600080fd5b34801561006857600080fd5b506100716100cb565b005b34801561007f57600080fd5b5061009e60048036038101908080359060200190929190505050610116565b005b3480156100ac57600080fd5b506100b561017e565b6040518082815260200191505060405180910390f35b600160008082825401925050819055507f20d8a6f5a693f9d1d627a598e8820f7a55ee74c183aa8f1a30e8d4e8dd9a8d846000546040518082815260200191505060405180910390a1565b60008090505b8181101561017a57600160008082825401925050819055507f20d8a6f5a693f9d1d627a598e8820f7a55ee74c183aa8f1a30e8d4e8dd9a8d846000546040518082815260200191505060405180910390a1808060010191505061011c565b5050565b600080549050905600a165627a7a7230582014739e9b3a5a1416c38c575513b4825a6bcf30121c099700b1f2988752659d3b0029").unwrap();

    let (_, contract) = client.create_confidential_contract(counter_code, &U256::zero());

    // Now, make a call to the getCounter method and ensure it's correct, i.e. zero.
    // Note that this uses the highest level API available in the runtime.
    let counter_zero = client.confidential_call(&contract, get_counter_data(), &U256::zero());
    let expected_zero = vec![
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];

    assert_eq!(counter_zero, expected_zero);

    contract
}

/// Returns the sighash of the getCounter method.
fn get_counter_data() -> Vec<u8> {
    hex::decode("8ada066e").unwrap()
}

/// Invokes the `incrementCounter` method on the contract (and does some post
/// validation to sanity check it worked).
fn increment_counter<'a>(contract: Address, client: &mut MutexGuard<'a, test::Client>) {
    let increment_counter_data = hex::decode("5b34b966").unwrap();
    client.confidential_send(Some(&contract), increment_counter_data, &U256::zero());

    let counter_one = client.confidential_call(&contract, get_counter_data(), &U256::zero());
    let expected_one = vec![
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 1,
    ];

    assert_eq!(counter_one, expected_one);
}

/// Peers directly into storage and asserts that the storage value associated
/// with the `_counter` variable in the contract is None. It should be None
/// because at this point, only the constructor has run, which does nothing
/// to set the _counter variable and so is not encrypted.
fn validate_counter_storage_is_none<'a>(
    contract: Address,
    client: &mut MutexGuard<'a, test::Client>,
) {
    let km_confidential_ctx = client.key_manager_confidential_ctx(contract.clone());
    let storage_key = H256::from(0);

    test::with_batch_handler(|ctx| {
        let ectx = ctx.runtime
            .downcast_mut::<runtime_ethereum::EthereumContext>()
            .unwrap();
        let state = ectx.cache.get_state(km_confidential_ctx.clone()).unwrap();

        // transparently decrypted with the state's injected confidential ctx
        let unencrypted_storage_counter = state.storage_at(&contract, &storage_key).unwrap();

        assert_eq!(unencrypted_storage_counter, H256::from(0));

        let encrypted_key = state.to_storage_key(&storage_key);
        let encrypted_storage_counter = state._storage_at(&contract, &encrypted_key).unwrap();

        assert_eq!(encrypted_storage_counter, None);
    });
}

/// Peers directly into storage and asserts that the storage value associated
/// with the `_counter` variable in the contract is the value 1 *encrypted*.
fn validate_counter_storage_is_encrypted<'a>(
    contract: Address,
    client: &mut MutexGuard<'a, test::Client>,
    expected_counter_value: H256,
) {
    test::with_batch_handler(|ctx| {
        let km_confidential_ctx = client.key_manager_confidential_ctx(contract.clone());
        let storage_key = H256::from(0);
        let ectx = ctx.runtime
            .downcast_mut::<runtime_ethereum::EthereumContext>()
            .unwrap();
        let state = ectx.cache.get_state(km_confidential_ctx.clone()).unwrap();

        // First validate the storage from the top level state api.
        // State::storage_at transparently decrypts with the state's injected confidential ctx.
        let unencrypted_storage_counter = state.storage_at(&contract, &storage_key).unwrap();
        assert_eq!(unencrypted_storage_counter, expected_counter_value);

        // Second, let's peek under the hood to look at the encrypted state.
        let encrypted_key = keccak_hash::keccak(&km_confidential_ctx
            .encrypt_storage(storage_key.to_vec())
            .unwrap());
        // State::_storage_at gives the raw underlying storage without decrypting.
        let encrypted_storage_counter = state
            ._storage_at(&contract, &encrypted_key)
            .unwrap()
            .unwrap();

        let ctx_decrypted_storage_counter = km_confidential_ctx
            .decrypt_storage(encrypted_storage_counter.clone())
            .unwrap();

        // Encrypted storage's length should be expanded from the original value.
        assert_eq!(encrypted_storage_counter.len(), 48);
        // Decryption should be of size H256.
        assert_eq!(ctx_decrypted_storage_counter.len(), 32);
        // Finally ensure the correct value of 1 is stored.
        assert_eq!(
            H256::from_slice(&ctx_decrypted_storage_counter[..32]),
            expected_counter_value,
        );
    });
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
/// this tests that storage is correctly encrypted when storage
/// is set in the initcode. It does this by
///
/// - deploying the contract
/// - validating the encrypted storage is set after deployment.
///
#[test]
fn test_contract_storage_encryption_initcode() {
    let mut client = test::Client::instance();
    let contract = deploy_counter_with_initcode_storage(&mut client);
    validate_counter_storage_is_encrypted(contract, &mut client, H256::from(5));
}

/// Deploys the contract with a single argument: 5.
fn deploy_counter_with_initcode_storage<'a>(client: &mut MutexGuard<'a, test::Client>) -> Address {
    let initcode =
        hex::decode("608060405234801561001057600080fd5b506040516020806100ea83398101806040528101908080519060200190929190505050806000819055505060a1806100496000396000f300608060405260043610603f576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff1680638ada066e146044575b600080fd5b348015604f57600080fd5b506056606c565b6040518082815260200191505060405180910390f35b600080549050905600a165627a7a72305820137067541d27b3ea9965691d1cb4585098dddc2cc08809233b6a1df18ddc110300290000000000000000000000000000000000000000000000000000000000000005").unwrap();

    let (_, contract) = client.create_confidential_contract(initcode, &U256::zero());

    // Now, make a call to the getCounter method and ensure it's correct, i.e. five.
    // Note that this uses the highest level API available in the runtime.
    let counter_five = client.confidential_call(&contract, get_counter_data(), &U256::zero());
    let expected_five = vec![
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 5,
    ];

    assert_eq!(counter_five, expected_five);

    contract
}
