#![feature(use_extern_macros)]
#![feature(alloc)]

mod evm;

extern crate protobuf;

extern crate alloc;
extern crate bigint;
extern crate hexutil;
extern crate sha3;
extern crate sputnikvm;

extern crate ekiden_core;
extern crate ekiden_trusted;

extern crate evm_api;

use evm_api::{with_api, EthState, ExecuteTransactionRequest, ExecuteTransactionResponse,
              InitStateRequest, InitStateResponse, Transaction};

use sputnikvm::{TransactionAction, ValidTransaction};

use bigint::{Address, Gas, H256, U256};
use hexutil::{read_hex, to_hex};

use std::rc::Rc;
use std::str::FromStr;

use evm::fire_transactions_and_update_state;

use ekiden_core::error::Result;
use ekiden_trusted::db::database_schema;
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

// Create database schema.
database_schema! {
    pub struct Db {
        pub state: evm_api::EthState,
    }
}

fn init_genesis_state(_request: &InitStateRequest) -> Result<InitStateResponse> {
    println!("*** Init genesis state");
    let response = InitStateResponse::new();
    let db = Db::new();
    db.state.insert(&EthState::new());
    Ok(response)
}

fn to_valid_transaction(transaction: &Transaction) -> ValidTransaction {
    let action = if transaction.get_is_call() {
        TransactionAction::Call(Address::from_str(transaction.get_address().clone()).unwrap())
    } else {
        TransactionAction::Create
    };

    ValidTransaction {
        caller: Some(Address::from_str(transaction.get_caller().clone()).unwrap()),
        action: action,
        gas_price: Gas::zero(),
        gas_limit: Gas::max_value(),
        value: U256::zero(),
        input: Rc::new(read_hex(transaction.get_input()).unwrap()),
        nonce: U256::zero(),
    }
}

fn execute_transaction(request: &ExecuteTransactionRequest) -> Result<ExecuteTransactionResponse> {
    println!("*** Execute transaction");
    println!("Transaction: {:?}", request.get_transaction());

    let db = Db::new();
    let state = db.state.get().unwrap();

    let transactions = [to_valid_transaction(request.get_transaction())];
    let (new_state, _) = fire_transactions_and_update_state(&transactions, &state, 1);

    db.state.insert(&new_state);

    let mut response = ExecuteTransactionResponse::new();

    // TODO: set from vm.status (VMStatus::ExitedOk = true)
    response.set_status(true);

    Ok(response)
}
