#[macro_use]
extern crate assert_matches;
extern crate ethcore;
extern crate ethereum_types;
extern crate ethkey;
extern crate hex;
extern crate oasis_runtime;
extern crate oasis_runtime_common;

use ekiden_runtime::transaction::dispatcher::CheckOnlySuccess;
use ethcore::{
    rlp,
    transaction::{Action, Transaction as EthcoreTransaction},
};
use ethereum_types::{H256, U256};
use oasis_runtime::{methods, test};

#[test]
fn test_create_balance() {
    let mut client = test::Client::new();

    let init_bal = client.balance(&client.keypair.address());
    let init_nonce = client.nonce(&client.keypair.address());

    let code = hex::decode("3331600055").unwrap(); // SSTORE(0x0, BALANCE(CALLER()))
    let contract_bal = U256::from(10);
    let (tx_hash, contract) = client.create_contract(code, &contract_bal);
    let receipt = client.result(tx_hash);
    let gas_used = receipt.gas_used;

    // Sender's remaining balance should be initial balance - contract balance - gas fee.
    let expected_remaining_bal = init_bal - contract_bal - gas_used * client.gas_price;

    // Check that sender's balance was updated correctly.
    let remaining_bal = client.balance(&client.keypair.address());
    assert_eq!(remaining_bal, expected_remaining_bal);

    // Check that sender's nonce was updated.
    let nonce = client.nonce(&client.keypair.address());
    assert_eq!(nonce, init_nonce + U256::one());

    // Check that contract balance was updated correctly.
    let bal = client.balance(&contract);
    assert_eq!(bal, contract_bal);

    // Check that sender balance during deploy transaction was:
    // initial balance - contract balance - gas limit * gas price
    let value = client.raw_storage(contract, H256::zero()).unwrap();
    assert_eq!(
        H256::from(&value[..]),
        H256::from(init_bal - contract_bal - client.gas_limit * client.gas_price)
    );
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

    let mut client = test::Client::new();

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
    let mut client = test::Client::new();

    let contract_code = hex::decode("6080604052348015600f57600080fd5b50609c8061001e6000396000f300608060405260043610603e5763ffffffff7c0100000000000000000000000000000000000000000000000000000000600035041663346fb5c981146043575b600080fd5b348015604e57600080fd5b506058600435606a565b60408051918252519081900360200190f35b600101905600a165627a7a72305820ea09447c835e5eb442e1a85e271b0ae6decf8551aa73948ab6b53e8dd1fa0dca0029").unwrap();

    // deploy once
    let (hash, _contract) = client.create_contract(contract_code.clone(), &U256::zero());
    let receipt = client.result(hash);
    let status = receipt.status_code;
    assert_eq!(status, 1);

    // deploy again
    let (hash, _contract) = client.create_contract(contract_code.clone(), &U256::zero());
    let receipt = client.result(hash);
    let status = receipt.status_code;
    assert_eq!(status, 1);
}

#[test]
fn test_nonce_checking() {
    let mut client = test::Client::new();

    // Nonce should start at 0.
    let nonce = client.nonce(&client.keypair.address());
    assert_eq!(nonce, U256::zero());

    // Send a transaction with nonce 0.
    let (tx_hash, _) = client
        .send(None, vec![], &U256::zero(), Some(U256::zero()))
        .expect("transaction should succeed");
    let receipt = client.result(tx_hash);
    let status = receipt.status_code;
    assert_eq!(status, 1);

    // Nonce should be 1 after a successful transaction.
    let nonce = client.nonce(&client.keypair.address());
    assert_eq!(nonce, U256::from(1));

    // Try to send a transaction with invalid nonce (should fail).
    let result = client.send(None, vec![], &U256::zero(), Some(U256::from(100)));
    assert!(result.is_err());

    // Nonce should still be 1 after a failed transaction.
    let nonce = client.nonce(&client.keypair.address());
    assert_eq!(nonce, U256::from(1));
}

#[test]
fn test_signature_verification() {
    let mut client = test::Client::new();

    let bad_sig = EthcoreTransaction {
        action: Action::Create,
        nonce: client.nonce(&client.keypair.address()),
        gas_price: U256::from(0),
        gas: U256::from(1000000),
        value: U256::from(0),
        data: vec![],
    }
    .fake_sign(client.keypair.address());
    let check_should_fail = client
        .check_batch(|_client, ctx| methods::execute::tx(&rlp::encode(&bad_sig).into_vec(), ctx));
    let good_sig = EthcoreTransaction {
        action: Action::Create,
        nonce: client.nonce(&client.keypair.address()),
        gas_price: U256::from(1),
        gas: U256::from(1000000),
        value: U256::from(0),
        data: vec![],
    }
    .sign(client.keypair.secret(), None);
    let check_should_pass = client
        .check_batch(|_client, ctx| methods::execute::tx(&rlp::encode(&good_sig).into_vec(), ctx));

    // Expected result: Err(InvalidSignature).
    match check_should_fail {
        Err(error) => {
            let ethkey_error = error.downcast::<ethkey::Error>().unwrap();
            assert_matches!(ethkey_error, ethkey::Error::InvalidSignature);
        }
        _ => assert!(false),
    }

    // Expected result: Err(CheckOnlySuccess).
    match check_should_pass {
        Err(error) => {
            assert!(error.downcast_ref::<CheckOnlySuccess>().is_some());
        }
        _ => assert!(false),
    }
}
