#![feature(use_extern_macros)]
#![feature(alloc)]

mod evm;
mod util;

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

use evm_api::{with_api, AccountBalanceResponse, AccountNonceResponse, AccountRequest,
              ExecuteRawTransactionRequest, ExecuteTransactionRequest, ExecuteTransactionResponse,
              InitStateRequest, InitStateResponse, TransactionRecordRequest,
              TransactionRecordResponse};

use sputnikvm::{VMStatus, VM};
use sputnikvm_network_classic::MainnetEIP160Patch;

use bigint::{Address, H256};
use block::Transaction;
use hexutil::read_hex;
use sha3::{Digest, Keccak256};

use std::str;

use evm::{fire_transaction, get_balance, get_nonce, save_transaction_record, update_state_from_vm,
          StateDb};

use ekiden_core::error::{Error, Result};
use ekiden_trusted::contract::create_contract;
use ekiden_trusted::enclave::enclave_init;
use ekiden_trusted::key_manager::use_key_manager_contract;

use rlp::UntrustedRlp;

use util::{normalize_hex_str, to_valid, to_valid_unsigned, unsigned_transaction_hash};

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
        let mut account = account_state.clone();
        account.set_address(normalize_hex_str(account_state.get_address()));
        state.accounts.insert(account.get_address(), &account);
    }

    state.genesis_initialized.insert(&true);
    Ok(InitStateResponse::new())
}

fn get_transaction_record(request: &TransactionRecordRequest) -> Result<TransactionRecordResponse> {
    println!("*** Get transaction record");
    println!("Hash: {:?}", request.get_hash());

    let hash = normalize_hex_str(request.get_hash());

    let mut response = TransactionRecordResponse::new();

    let state = StateDb::new();
    match state.transactions.get(&hash) {
        Some(b) => response.set_record(b),
        None => {}
    };

    Ok(response)
}

fn get_account_balance(request: &AccountRequest) -> Result<AccountBalanceResponse> {
    println!("*** Get account balance");
    println!("Address: {:?}", request.get_address());

    let address = normalize_hex_str(request.get_address());
    let balance = get_balance(address);

    let mut response = AccountBalanceResponse::new();
    response.set_balance(format!("{}", balance));

    Ok(response)
}

fn get_account_nonce(request: &AccountRequest) -> Result<AccountNonceResponse> {
    println!("*** Get account nonce");
    println!("Address: {:?}", request.get_address());

    let address = normalize_hex_str(request.get_address());
    let nonce = get_nonce(address);

    let mut response = AccountNonceResponse::new();
    response.set_nonce(format!("{}", nonce));

    Ok(response)
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
    let transaction: Transaction = rlp.as_val().unwrap();

    let valid = match to_valid::<MainnetEIP160Patch>(&transaction) {
        Ok(val) => val,
        Err(err) => return Err(Error::new(format!("{:?}", err))),
    };

    let vm = fire_transaction(&valid, 1);
    update_state_from_vm(&vm);
    // TODO: block number, from and to addresses
    save_transaction_record(
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

fn simulate_transaction(request: &ExecuteTransactionRequest) -> Result<ExecuteTransactionResponse> {
    println!("*** Simulate transaction");
    println!("Transaction: {:?}", request.get_transaction());

    let valid = to_valid_unsigned(request.get_transaction());

    let vm = fire_transaction(&valid, 1);
    let mut response = ExecuteTransactionResponse::new();

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

// WARNING: FOR DEVELOPMENT+TESTING ONLY. DISABLE IN PRODUCTION!
// executes an unsigned transaction from a web3 sendTransaction
// no validation is performed
fn debug_execute_unsigned_transaction(
    request: &ExecuteTransactionRequest,
) -> Result<ExecuteTransactionResponse> {
    println!("*** Execute transaction");
    println!("Transaction: {:?}", request.get_transaction());

    let valid = to_valid_unsigned(request.get_transaction());
    let hash = unsigned_transaction_hash(&valid);

    let vm = fire_transaction(&valid, 1);
    update_state_from_vm(&vm);

    // TODO: block number, from and to addresses
    save_transaction_record(
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
