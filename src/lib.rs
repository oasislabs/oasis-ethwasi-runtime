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
use block::Transaction;
use hexutil::{read_hex, to_hex};
use sha3::{Digest, Keccak256};

use std::rc::Rc;
use std::str;
use std::str::FromStr;

use evm::{fire_transaction, get_balance, get_nonce, update_state_from_vm};

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

fn init_genesis_state(_request: &InitStateRequest) -> Result<InitStateResponse> {
    println!("*** Init genesis state");

    /*
    let mut genesis = Vec::new();
    // add account address 7110316b618d20d0c44728ac2a3d683536ea682b. TODO: move this to a genesis config file
    genesis.push((SecretKey::from_slice(&SECP256K1, &read_hex("533d62aea9bbcb821dfdda14966bb01bfbbb53b7e9f5f0d69b8326e052e3450c").unwrap()).unwrap(), U256::from_dec_str("200000000000000000000").unwrap()));
    let miner_state = miner::make_state::<ByzantiumPatch>(genesis);
    */

    let response = InitStateResponse::new();
    // TODO: insert genesis state
    //let db = Db::new();
    //db.state.insert(&EthState::new());
    Ok(response)
}

// validates transaction and returns a ValidTransaction on success
fn to_valid<P: Patch>(
    transaction: Transaction,
) -> ::std::result::Result<ValidTransaction, PreExecutionError> {
    // debugging
    println!("*** Validate block transaction");
    println!("Data: {:?}", transaction);

    // check caller signature
    let caller = match transaction.caller() {
        Ok(val) => val,
        Err(_) => return Err(PreExecutionError::InvalidCaller),
    };
    let caller_str = caller.to_string();

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

    let valid = match to_valid::<MainnetEIP160Patch>(transaction) {
        Ok(val) => val,
        Err(err) => return Err(Error::new(format!("{:?}", err))),
    };

    let vm = fire_transaction(&valid, 1);
    update_state_from_vm(&vm);

    let mut response = ExecuteTransactionResponse::new();
    response.set_hash(format!("{:x}", hash));
    Ok(response)
}

fn execute_transaction(request: &ExecuteTransactionRequest) -> Result<ExecuteTransactionResponse> {
    println!("*** Execute transaction");
    println!("Transaction: {:?}", request.get_transaction());

    let transaction = to_valid_transaction(request.get_transaction());
    let vm = fire_transaction(&transaction, 1);
    if !request.get_simulate() {
        println!(" Not eth_call, updating state");
        update_state_from_vm(&vm)
    } else {
        println!("eth_call, not updating state");
    }

    let mut response = ExecuteTransactionResponse::new();

    // TODO: set transaction hash
    response.set_hash(String::new());

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

    response.set_used_gas(format!("{:x}", vm.used_gas()));

    Ok(response)
}
