extern crate ekiden_common;
extern crate ekiden_core;
extern crate ekiden_keymanager_common;
extern crate ekiden_roothash_base;
extern crate ekiden_storage_base;
extern crate ekiden_storage_dummy;
extern crate ekiden_trusted;
extern crate ethcore;
extern crate ethereum_types;
extern crate ethkey;
extern crate hex;
extern crate runtime_ethereum;
extern crate runtime_ethereum_common;

use ekiden_trusted::db::{Database, DatabaseHandle};
use ethcore::{
    rlp,
    transaction::{Action, Transaction as EthcoreTransaction},
};
use ethereum_types::{H256, U256};
use runtime_ethereum::test;

#[test]
fn test_create_balance() {
    let mut client = test::Client::instance();

    let init_bal =
        runtime_ethereum::get_account_balance(&client.keypair.address(), &mut test::dummy_ctx())
            .unwrap();
    let contract_bal = U256::from(10);
    let remaining_bal = init_bal - contract_bal;

    let init_nonce =
        runtime_ethereum::get_account_nonce(&client.keypair.address(), &mut test::dummy_ctx())
            .unwrap();

    let code = hex::decode("3331600055").unwrap(); // SSTORE(0x0, BALANCE(CALLER()))
    let (_, contract) = client.create_contract(code, &contract_bal);

    assert_eq!(
        runtime_ethereum::get_account_balance(&client.keypair.address(), &mut test::dummy_ctx())
            .unwrap(),
        remaining_bal
    );
    assert_eq!(
        runtime_ethereum::get_account_nonce(&client.keypair.address(), &mut test::dummy_ctx())
            .unwrap(),
        init_nonce + U256::one()
    );
    assert_eq!(
        runtime_ethereum::get_account_balance(&contract, &mut test::dummy_ctx()).unwrap(),
        contract_bal
    );
    assert_eq!(
        runtime_ethereum::get_storage_at(&(contract, H256::zero()), &mut test::dummy_ctx())
            .unwrap(),
        H256::from(&remaining_bal)
    );
}

#[test]
fn test_solidity_blockhash() {
    // pragma solidity ^0.4.18;
    // contract The {
    //   function hash(uint64 num) public view returns (bytes32) {
    //     return blockhash(num);
    //   }
    // }

    use std::mem::transmute;

    let mut client = test::Client::instance();
    let blockhash_code = hex::decode("608060405234801561001057600080fd5b5060d58061001f6000396000f300608060405260043610603f576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063e432a10e146044575b600080fd5b348015604f57600080fd5b506076600480360381019080803567ffffffffffffffff1690602001909291905050506094565b60405180826000191660001916815260200191505060405180910390f35b60008167ffffffffffffffff164090509190505600a165627a7a7230582078c16bf994a1597df9b750bb680f3fc4b4e8c9c8f51607bbfcc28d9496a211d70029").unwrap();

    let (_, contract) = client.create_contract(blockhash_code, &U256::zero());

    let mut blockhash = |num: u64| -> Vec<u8> {
        let mut data =
            hex::decode("e432a10e0000000000000000000000000000000000000000000000000000000000000000")
                .unwrap();
        let bytes: [u8; 8] = unsafe { transmute(num.to_be()) };
        for i in 0..8 {
            data[28 + i] = bytes[i];
        }
        client.call(&contract, data, &U256::zero())
    };

    let block_number = test::with_batch_handler(|ctx| {
        let ectx = ctx
            .runtime
            .downcast_mut::<runtime_ethereum::EthereumContext>()
            .unwrap();
        ectx.cache.get_latest_block_number()
    });
    let client_blockhash = blockhash(block_number);

    test::with_batch_handler(|ctx| {
        let ectx = ctx
            .runtime
            .downcast_mut::<runtime_ethereum::EthereumContext>()
            .unwrap();
        assert_eq!(
            client_blockhash,
            ectx.cache
                .block_hash(ectx.cache.get_latest_block_number())
                .unwrap()
                .to_vec()
        );
    });
}

#[test]
fn test_solidity_x_contract_call() {
    // contract A {
    //   function call_a(address b, int a) public pure returns (int) {
    //       B cb = B(b);
    //       return cb.call_b(a);
    //     }
    // }
    //
    // contract B {
    //     function call_b(int b) public pure returns (int) {
    //             return b + 1;
    //         }
    // }

    let mut client = test::Client::instance();

    let contract_a_code = hex::decode("608060405234801561001057600080fd5b5061015d806100206000396000f3006080604052600436106100405763ffffffff7c0100000000000000000000000000000000000000000000000000000000600035041663e3f300558114610045575b600080fd5b34801561005157600080fd5b5061007673ffffffffffffffffffffffffffffffffffffffff60043516602435610088565b60408051918252519081900360200190f35b6000808390508073ffffffffffffffffffffffffffffffffffffffff1663346fb5c9846040518263ffffffff167c010000000000000000000000000000000000000000000000000000000002815260040180828152602001915050602060405180830381600087803b1580156100fd57600080fd5b505af1158015610111573d6000803e3d6000fd5b505050506040513d602081101561012757600080fd5b50519493505050505600a165627a7a7230582062a004e161bd855be0a78838f92bafcbb4cef5df9f9ac673c2f7d174eff863fb0029").unwrap();
    let (_, contract_a) = client.create_contract(contract_a_code, &U256::zero());

    let contract_b_code = hex::decode("6080604052348015600f57600080fd5b50609c8061001e6000396000f300608060405260043610603e5763ffffffff7c0100000000000000000000000000000000000000000000000000000000600035041663346fb5c981146043575b600080fd5b348015604e57600080fd5b506058600435606a565b60408051918252519081900360200190f35b600101905600a165627a7a72305820ea09447c835e5eb442e1a85e271b0ae6decf8551aa73948ab6b53e8dd1fa0dca0029").unwrap();
    let (_, contract_b) = client.create_contract(contract_b_code, &U256::zero());

    let data = hex::decode(format!(
        "e3f30055000000000000000000000000{:\
         x}0000000000000000000000000000000000000000000000000000000000000029",
        contract_b
    ))
    .unwrap();
    let output = client.call(&contract_a, data, &U256::zero());

    // expected output is 42
    assert_eq!(
        hex::encode(output),
        "000000000000000000000000000000000000000000000000000000000000002a"
    );
}

#[test]
fn test_redeploy() {
    let mut client = test::Client::instance();

    let contract_code = hex::decode("6080604052348015600f57600080fd5b50609c8061001e6000396000f300608060405260043610603e5763ffffffff7c0100000000000000000000000000000000000000000000000000000000600035041663346fb5c981146043575b600080fd5b348015604e57600080fd5b506058600435606a565b60408051918252519081900360200190f35b600101905600a165627a7a72305820ea09447c835e5eb442e1a85e271b0ae6decf8551aa73948ab6b53e8dd1fa0dca0029").unwrap();

    // deploy once
    let (hash, _contract) = client.create_contract(contract_code.clone(), &U256::zero());
    let receipt = runtime_ethereum::get_receipt(&hash, &mut test::dummy_ctx())
        .unwrap()
        .unwrap();
    let status = receipt.status_code.unwrap();
    assert_eq!(status, 1 as u64);

    // deploy again
    let (hash, _contract) = client.create_contract(contract_code.clone(), &U256::zero());
    let receipt = runtime_ethereum::get_receipt(&hash, &mut test::dummy_ctx())
        .unwrap()
        .unwrap();
    let status = receipt.status_code.unwrap();
    assert_eq!(status, 1 as u64);
}

#[test]
fn test_signature_verification() {
    let client = test::Client::instance();

    let bad_sig = EthcoreTransaction {
        action: Action::Create,
        nonce: runtime_ethereum::get_account_nonce(
            &client.keypair.address(),
            &mut test::dummy_ctx(),
        )
        .unwrap(),
        gas_price: U256::from(0),
        gas: U256::from(1000000),
        value: U256::from(0),
        data: vec![],
    }
    .fake_sign(client.keypair.address());
    let bad_result = runtime_ethereum::execute_raw_transaction(
        &rlp::encode(&bad_sig).into_vec(),
        &mut test::dummy_ctx(),
    )
    .unwrap()
    .hash;
    let good_sig = EthcoreTransaction {
        action: Action::Create,
        nonce: runtime_ethereum::get_account_nonce(
            &client.keypair.address(),
            &mut test::dummy_ctx(),
        )
        .unwrap(),
        gas_price: U256::from(0),
        gas: U256::from(1000000),
        value: U256::from(0),
        data: vec![],
    }
    .sign(client.keypair.secret(), None);
    let good_result = runtime_ethereum::execute_raw_transaction(
        &rlp::encode(&good_sig).into_vec(),
        &mut test::dummy_ctx(),
    )
    .unwrap()
    .hash;
    assert!(bad_result.is_err());
    assert!(good_result.is_ok());
}

#[test]
fn test_last_hashes() {
    let mut client = test::Client::instance();

    // ensure that we have >256 blocks
    for _i in 0..260 {
        client.create_contract(vec![], &U256::zero());
    }

    // get last_hashes from latest block
    test::with_batch_handler(|ctx| {
        let ectx = ctx
            .runtime
            .downcast_mut::<runtime_ethereum::EthereumContext>()
            .unwrap();

        let last_hashes = ectx
            .cache
            .last_hashes(&ectx.cache.best_block_header().hash());

        assert_eq!(last_hashes.len(), 256);
        assert_eq!(
            last_hashes[1],
            ectx.cache
                .block_hash(ectx.cache.get_latest_block_number() - 1)
                .unwrap()
        );
    });
}

#[test]
fn test_cache_invalidation() {
    let mut client = test::Client::instance();

    // Perform initial transaction to get a valid state root.
    let code = hex::decode("3331600055").unwrap(); // SSTORE(0x0, BALANCE(CALLER()))
    let (_, address_1) = client.create_contract(code.clone(), &U256::from(42));
    let state_root_1 = DatabaseHandle::instance().get_root_hash();

    // Perform another transaction to get another state root.
    let (_, address_2) = client.create_contract(code, &U256::from(21));

    // Ensure both contracts exist.
    let best_block = test::with_batch_handler(|ctx| {
        assert_eq!(
            runtime_ethereum::get_account_balance(&address_1, ctx),
            Ok(U256::from(42))
        );
        assert_eq!(
            runtime_ethereum::get_account_balance(&address_2, ctx),
            Ok(U256::from(21))
        );
        let ectx = ctx
            .runtime
            .downcast_mut::<runtime_ethereum::EthereumContext>()
            .unwrap();
        ectx.cache.best_block_header().number()
    });

    // Simulate batch rolling back.
    DatabaseHandle::instance()
        .set_root_hash(state_root_1)
        .unwrap();

    // Ensure cache is invalidated correctly.
    test::with_batch_handler(|ctx| {
        assert_eq!(
            runtime_ethereum::get_account_balance(&address_1, ctx),
            Ok(U256::from(42))
        );
        assert_eq!(
            runtime_ethereum::get_account_balance(&address_2, ctx),
            Ok(U256::zero())
        );
        let ectx = ctx
            .runtime
            .downcast_mut::<runtime_ethereum::EthereumContext>()
            .unwrap();
        assert_eq!(best_block, ectx.cache.best_block_header().number() + 1)
    });
}
