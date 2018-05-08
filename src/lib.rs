#![feature(use_extern_macros)]
#![feature(alloc)]

mod evm;
//mod miner;

extern crate protobuf;

extern crate alloc;
extern crate bigint;
extern crate block;
extern crate hexutil;
extern crate sha3;
extern crate sputnikvm;

extern crate ekiden_core;
extern crate ekiden_trusted;

extern crate evm_api;

extern crate rlp;

extern crate sputnikvm_network_classic;

use evm_api::{with_api, ExecuteRawTransactionRequest, ExecuteTransactionRequest,
              ExecuteTransactionResponse, InitStateRequest, InitStateResponse,
              Transaction as EVMTransaction};

use sputnikvm::{Patch, PreExecutionError, TransactionAction, VMStatus, ValidTransaction, VM};
use sputnikvm_network_classic::MainnetEIP160Patch;

use bigint::{Address, Gas, H256, U256};
use block::{RlpHash, Transaction, TransactionSignature};
use hexutil::{read_hex, to_hex};
use sha3::{Digest, Keccak256};

use std::rc::Rc;
use std::str;
use std::str::FromStr;

use evm::{fire_transaction, get_balance, get_nonce, store_receipt, update_state_from_vm, StateDb};

use ekiden_core::error::{Error, Result};
use ekiden_trusted::contract::create_contract;
use ekiden_trusted::enclave::enclave_init;
use ekiden_trusted::key_manager::use_key_manager_contract;

use rlp::UntrustedRlp;

enclave_init!();

// Configure the key manager contract to use.
use_key_manager_contract!("generated/key-manager.identity");

// Create enclave contract interface.
with_api! {
    create_contract!(api);
}

fn genesis_block_initialized(request: &bool) -> Result<bool> {
    Ok(StateDb::new().genesis_initialized.is_present())
}

// TODO: secure this method so it can't be called by any client.
fn init_genesis_block(block: &InitStateRequest) -> Result<InitStateResponse> {
    println!("*** Init genesis block");
    let state = StateDb::new();

    if state.genesis_initialized.is_present() {
        return Err(Error::new("Genesis block already created"));
    }

    // Insert account states from genesis block
    for account_state in block.get_accounts() {
        // remove "0x" prefix and lowercase address
        let mut account = account_state.clone();
        let address = account_state
            .get_address()
            .trim_left_matches("0x")
            .to_lowercase();
        account.set_address(address);

        state.accounts.insert(account.get_address(), &account);
    }

    state.genesis_initialized.insert(&true);
    Ok(InitStateResponse::new())
}

// validates transaction and returns a ValidTransaction on success
fn to_valid<P: Patch>(
    transaction: &Transaction,
) -> ::std::result::Result<ValidTransaction, PreExecutionError> {
    // debugging
    println!("*** Validate block transaction");
    println!("Data: {:?}", transaction);

    // check caller signature
    let caller = match transaction.caller() {
        Ok(val) => val,
        Err(_) => return Err(PreExecutionError::InvalidCaller),
    };
    let caller_str = caller.hex();

    // check nonce
    // TODO: what if account doesn't exist?
    let nonce = get_nonce(caller_str.clone());
    if nonce != transaction.nonce {
        return Err(PreExecutionError::InvalidNonce);
    }

    let valid = ValidTransaction {
        caller: Some(caller),
        gas_price: transaction.gas_price,
        gas_limit: transaction.gas_limit,
        action: transaction.action.clone(),
        value: transaction.value,
        input: Rc::new(transaction.input.clone()),
        nonce: nonce,
    };

    // check gas limit
    if valid.gas_limit < valid.intrinsic_gas::<P>() {
        return Err(PreExecutionError::InsufficientGasLimit);
    }

    // check balance
    // TODO: what if account doesn't exist?
    let balance = get_balance(caller_str);

    let gas_limit: U256 = valid.gas_limit.into();
    let gas_price: U256 = valid.gas_price.into();

    let (preclaimed_value, overflowed1) = gas_limit.overflowing_mul(gas_price);
    let (total, overflowed2) = preclaimed_value.overflowing_add(valid.value);
    if overflowed1 || overflowed2 {
        return Err(PreExecutionError::InsufficientBalance);
    }

    if balance < total {
        return Err(PreExecutionError::InsufficientBalance);
    }

    Ok(valid)
}

fn to_valid_transaction(transaction: &EVMTransaction) -> ValidTransaction {
    let action = if transaction.get_is_call() {
        TransactionAction::Call(Address::from_str(transaction.get_address().clone()).unwrap())
    } else {
        TransactionAction::Create
    };

    let caller_str = transaction.get_caller();

    // TODO: verify that nonce matches?
    let nonce = if transaction.get_use_nonce() {
        U256::from_str(transaction.get_nonce().clone()).unwrap()
    } else {
        get_nonce(caller_str.to_string())
    };

    ValidTransaction {
        caller: Some(Address::from_str(caller_str.clone()).unwrap()),
        action: action,
        gas_price: Gas::zero(),
        gas_limit: Gas::max_value(),
        value: U256::zero(),
        input: Rc::new(read_hex(transaction.get_input()).unwrap()),
        nonce: nonce,
    }
}

// FOR DEVELOPMENT+TESTING ONLY
// computes transaction hash from an unsigned web3 sendTransaction/call
// signature is fake, but unique per account
fn unsigned_transaction_hash(transaction: &ValidTransaction) -> H256 {
    // unique per-account fake "signature"
    let signature = TransactionSignature {
        v: 0,
        r: H256::from(transaction.caller.unwrap()),
        s: H256::new(),
    };

    let block_transaction = Transaction {
        nonce: transaction.nonce,
        gas_price: transaction.gas_price,
        gas_limit: transaction.gas_limit,
        action: transaction.action,
        value: transaction.value,
        signature: signature,
        input: Rc::new(transaction.input.clone()).to_vec(),
    };

    block_transaction.rlp_hash()
}

fn execute_raw_transaction(
    request: &ExecuteRawTransactionRequest,
) -> Result<ExecuteTransactionResponse> {
    println!("*** Execute raw transaction");
    println!("Data: {:?}", request.get_data());

    let value = read_hex(request.get_data()).unwrap();
    let hash = H256::from(Keccak256::digest(&value).as_slice());

    let rlp = UntrustedRlp::new(&value);

    // TODO: handle errors
    let mut transaction: Transaction = rlp.as_val().unwrap();

    let valid = match to_valid::<MainnetEIP160Patch>(&transaction) {
        Ok(val) => val,
        Err(err) => return Err(Error::new(format!("{:?}", err))),
    };

    let vm = fire_transaction(&valid, 1);
    update_state_from_vm(&vm);
    // TODO: block number, from and to addresses
    store_receipt(
        hash,
        1.into(),
        0,
        Address::default(),
        Address::default(),
        &vm,
    );

    let mut response = ExecuteTransactionResponse::new();
    response.set_hash(format!("{:x}", hash));
    Ok(response)
}

fn execute_transaction(request: &ExecuteTransactionRequest) -> Result<ExecuteTransactionResponse> {
    println!("*** Execute transaction");
    println!("Transaction: {:?}", request.get_transaction());

    let valid = to_valid_transaction(request.get_transaction());
    let hash = unsigned_transaction_hash(&valid);

    let vm = fire_transaction(&valid, 1);
    if !request.get_simulate() {
        println!("Not eth_call, updating state");
        update_state_from_vm(&vm);

        // TODO: block number, from and to addresses
        store_receipt(
            hash,
            1.into(),
            0,
            Address::default(),
            Address::default(),
            &vm,
        );
    } else {
        println!("eth_call, not updating state");
    }

    let mut response = ExecuteTransactionResponse::new();

    response.set_hash(format!("{:x}", hash));

    // TODO: return error info to client
    match vm.status() {
        VMStatus::ExitedOk => response.set_status(true),
        _ => response.set_status(false),
    }

    let result = match str::from_utf8(&vm.out().to_vec()) {
        Ok(val) => val.to_string(),
        Err(_err) => String::new(),
    };
    response.set_result(result);

    response.set_used_gas(format!("{:?}", vm.used_gas()));

    Ok(response)
}
