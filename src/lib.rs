#![feature(use_extern_macros)]
#![feature(alloc)]

mod evm;
//mod miner;

extern crate protobuf;

extern crate alloc;
extern crate bigint;
extern crate hexutil;
extern crate sha3;
extern crate sputnikvm;

extern crate ekiden_core;
extern crate ekiden_trusted;

extern crate evm_api;

extern crate rlp;

use evm_api::{with_api, ExecuteTransactionRequest, ExecuteTransactionResponse, InitStateRequest,
              InitStateResponse, Transaction};

use sputnikvm::{TransactionAction, VMStatus, ValidTransaction, VM};

use bigint::{Address, Gas, H256, U256};
use hexutil::{read_hex, to_hex};

use std::rc::Rc;
use std::str;
use std::str::FromStr;

use evm::{fire_transaction, get_nonce, update_state_from_vm};

use ekiden_core::error::Result;
use ekiden_trusted::enclave::enclave_init;
use ekiden_trusted::key_manager::use_key_manager_contract;
use ekiden_trusted::rpc::create_enclave_rpc;

enclave_init!();

// Configure the key manager contract to use.
use_key_manager_contract!("generated/key-manager.identity");

// Create enclave RPC handlers.
with_api! {
    create_enclave_rpc!(api);
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

fn to_valid_transaction(transaction: &Transaction) -> ValidTransaction {
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

    Ok(response)
}
