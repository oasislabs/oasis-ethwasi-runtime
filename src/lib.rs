#![feature(use_extern_macros)]
#![feature(alloc)]

mod evm;
mod miner;
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

use evm_api::{with_api, AccountBalanceResponse, AccountCodeResponse, AccountNonceResponse,
              AccountRequest, Block, BlockRequest, BlockResponse, ExecuteRawTransactionRequest,
              ExecuteTransactionRequest, ExecuteTransactionResponse, InitStateRequest,
              InitStateResponse, TransactionRecordRequest, TransactionRecordResponse};

use sputnikvm::{VMStatus, VM};
use sputnikvm_network_classic::MainnetEIP160Patch;

use bigint::{Address, H256, U256};
use block::Transaction;
use hexutil::{read_hex, to_hex};
use sha3::{Digest, Keccak256};

use std::str;
use std::str::FromStr;

use evm::{fire_transaction, get_balance, get_code_string, get_nonce, save_transaction_record,
          update_state_from_vm, StateDb};

use miner::{get_block, get_latest_block_number, mine_block};

use ekiden_core::error::{Error, Result};
use ekiden_trusted::contract::create_contract;
use ekiden_trusted::enclave::enclave_init;
use ekiden_trusted::key_manager::use_key_manager_contract;

use rlp::UntrustedRlp;

use util::{normalize_hex_str, to_valid, unsigned_to_valid};

#[cfg(debug_assertions)]
use util::unsigned_transaction_hash;

enclave_init!();

// Configure the key manager contract to use.
use_key_manager_contract!("generated/key-manager.identity");

// Create enclave contract interface.
with_api! {
    create_contract!(api);
}

#[cfg(debug_assertions)]
fn genesis_block_initialized(request: &bool) -> Result<bool> {
    Ok(StateDb::new().genesis_initialized.is_present())
}

#[cfg(not(debug_assertions))]
fn genesis_block_initialized(request: &bool) -> Result<bool> {
    Err(Error::new("API available only in debug builds"))
}

// TODO: secure this method so it can't be called by any client.
#[cfg(debug_assertions)]
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

    // Mine block 0 with no transactions
    mine_block(None);

    state.genesis_initialized.insert(&true);
    Ok(InitStateResponse::new())
}

#[cfg(not(debug_assertions))]
fn init_genesis_block(block: &InitStateRequest) -> Result<InitStateResponse> {
    Err(Error::new("API available only in debug builds"))
}

fn get_block_by_number(request: &BlockRequest) -> Result<BlockResponse> {
    //println!("*** Get block by number");
    //println!("Request: {:?}", request);

    let number = if request.get_number() == "latest" {
        get_latest_block_number()
    } else {
        match U256::from_str(request.get_number()) {
            Ok(val) => val,
            Err(err) => return Err(Error::new(format!("{:?}", err))),
        }
    };

    let mut response = BlockResponse::new();

    let mut block = match get_block(number) {
        Some(val) => val,
        None => return Ok(response),
    };

    // if full transactions are requested, attach the TransactionRecord
    if request.get_full() {
        if let Some(val) = StateDb::new()
            .transactions
            .get(block.get_transaction_hash())
        {
            block.set_transaction(val);
        }
    }

    response.set_block(block);
    Ok(response)
}

fn get_transaction_record(request: &TransactionRecordRequest) -> Result<TransactionRecordResponse> {
    println!("*** Get transaction record");
    println!("Hash: {:?}", request.get_hash());

    let hash = normalize_hex_str(request.get_hash());

    let mut response = TransactionRecordResponse::new();

    let state = StateDb::new();
    if let Some(val) = state.transactions.get(&hash) {
        response.set_record(val);
    }

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

fn get_account_code(request: &AccountRequest) -> Result<AccountCodeResponse> {
    println!("*** Get account code");
    println!("Address: {:?}", request.get_address());

    let address = normalize_hex_str(request.get_address());
    let code = get_code_string(address);

    let mut response = AccountCodeResponse::new();
    response.set_code(code);

    Ok(response)
}

fn execute_raw_transaction(
    request: &ExecuteRawTransactionRequest,
) -> Result<ExecuteTransactionResponse> {
    println!("*** Execute raw transaction");
    println!("Data: {:?}", request.get_data());

    let value = match read_hex(request.get_data()) {
        Ok(val) => val,
        Err(err) => return Err(Error::new(format!("{:?}", err))),
    };
    let hash = H256::from(Keccak256::digest(&value).as_slice());

    let rlp = UntrustedRlp::new(&value);

    let transaction: Transaction = rlp.as_val()?;

    let valid = match to_valid::<MainnetEIP160Patch>(&transaction) {
        Ok(val) => val,
        Err(err) => return Err(Error::new(format!("{:?}", err))),
    };

    let vm = fire_transaction(&valid, 1);
    update_state_from_vm(&vm);
    let (block_number, block_hash) = mine_block(Some(hash));
    save_transaction_record(hash, block_hash, block_number, 0, valid, &vm);

    let mut response = ExecuteTransactionResponse::new();
    response.set_hash(format!("{:x}", hash));
    Ok(response)
}

fn simulate_transaction(request: &ExecuteTransactionRequest) -> Result<ExecuteTransactionResponse> {
    println!("*** Simulate transaction");
    println!("Transaction: {:?}", request.get_transaction());

    let valid = match unsigned_to_valid(request.get_transaction()) {
        Ok(val) => val,
        Err(err) => return Err(Error::new(format!("{:?}", err))),
    };

    let vm = fire_transaction(&valid, 1);
    let mut response = ExecuteTransactionResponse::new();

    // TODO: return error info to client
    match vm.status() {
        VMStatus::ExitedOk => response.set_status(true),
        _ => response.set_status(false),
    }

    let result = to_hex(&vm.out());
    println!("*** Result: {:?}", result);

    response.set_result(result);

    response.set_used_gas(format!("{:x}", vm.used_gas()));

    Ok(response)
}

// for debugging and testing: executes an unsigned transaction from a web3 sendTransaction
// attempts to execute the transaction without performing any validation
#[cfg(debug_assertions)]
fn debug_execute_unsigned_transaction(
    request: &ExecuteTransactionRequest,
) -> Result<ExecuteTransactionResponse> {
    println!("*** Execute transaction");
    println!("Transaction: {:?}", request.get_transaction());

    let valid = match unsigned_to_valid(request.get_transaction()) {
        Ok(val) => val,
        Err(err) => return Err(Error::new(format!("{:?}", err))),
    };

    let hash = unsigned_transaction_hash(&valid);

    let vm = fire_transaction(&valid, 1);
    update_state_from_vm(&vm);
    let (block_number, block_hash) = mine_block(Some(hash));
    save_transaction_record(hash, block_hash, block_number, 0, valid, &vm);

    let mut response = ExecuteTransactionResponse::new();
    response.set_hash(format!("{:x}", hash));

    Ok(response)
}

#[cfg(not(debug_assertions))]
fn debug_execute_unsigned_transaction(
    request: &ExecuteTransactionRequest,
) -> Result<ExecuteTransactionResponse> {
    Err(Error::new("API available only in debug builds"))
}
