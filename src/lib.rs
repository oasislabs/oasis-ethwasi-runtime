#![feature(use_extern_macros)]
#![feature(alloc)]

#[macro_use]
mod logger;
mod evm;
mod miner;
mod util;

extern crate log;
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
extern crate sputnikvm_network_foundation;

use evm_api::{with_api, AccountBalanceResponse, AccountCodeResponse, AccountNonceResponse,
              AccountRequest, BlockRequest, BlockResponse, ExecuteRawTransactionRequest,
              ExecuteTransactionRequest, InitStateRequest, InitStateResponse,
              InjectAccountsRequest, InjectAccountsResponse, SimulateTransactionResponse,
              TransactionHashResponse, TransactionRecordRequest, TransactionRecordResponse};

use sputnikvm::{VMStatus, VM};
use sputnikvm_network_classic::MainnetEIP160Patch;

use bigint::{Address, H256, U256};
use block::Transaction;
use hexutil::{read_hex, to_hex};
use sha3::{Digest, Keccak256};

use std::str::FromStr;

use evm::{fire_transaction, get_balance, get_code_string, get_nonce, save_transaction_record,
          update_state_from_vm, StateDb};

use evm::patch::ByzantiumPatch;

use miner::{get_block, get_latest_block_number, mine_block};

use ekiden_core::error::{Error, Result};
use ekiden_trusted::contract::create_contract;
use ekiden_trusted::enclave::enclave_init;

use rlp::UntrustedRlp;

use util::{to_valid, unsigned_to_valid};

#[cfg(debug_assertions)]
use util::unsigned_transaction_hash;

enclave_init!();

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
fn inject_accounts(request: &InjectAccountsRequest) -> Result<InjectAccountsResponse> {
    let state = StateDb::new();

    if state.genesis_initialized.is_present() {
        return Err(Error::new("Genesis block already created"));
    }

    // Insert account states
    for account in &request.accounts {
        state.accounts.insert(&account.address, &account);
    }

    Ok(InjectAccountsResponse {})
}

// TODO: secure this method so it can't be called by any client.
#[cfg(debug_assertions)]
fn init_genesis_block(_block: &InitStateRequest) -> Result<InitStateResponse> {
    info!("*** Init genesis block");
    let state = StateDb::new();

    if state.genesis_initialized.is_present() {
        return Err(Error::new("Genesis block already created"));
    }

    // Mine block 0 with no transactions
    mine_block(None);

    state.genesis_initialized.insert(&true);
    Ok(InitStateResponse {})
}

#[cfg(not(debug_assertions))]
fn init_genesis_block(block: &InitStateRequest) -> Result<InitStateResponse> {
    Err(Error::new("API available only in debug builds"))
}

/// TODO: first argument is ignored; remove once APIs support zero-argument signatures (#246)
fn get_block_height(request: &bool) -> Result<String> {
    Ok(format!("0x{:x}", get_latest_block_number()))
}

fn get_latest_block_hashes(block_height: &String) -> Result<Vec<String>> {
    let mut result = Vec::new();

    let current_block_height = get_latest_block_number();
    let mut next_start = match U256::from_str(block_height) {
        Ok(val) => val,
        Err(err) => return Err(Error::new(format!("{:?}", err))),
    };

    while next_start <= current_block_height {
        let transaction_hash = get_block(next_start).unwrap().transaction_hash.to_string();
        result.push(transaction_hash);
        next_start = next_start + U256::one();
    }

    Ok(result)
}

fn get_block_by_number(request: &BlockRequest) -> Result<BlockResponse> {
    //println!("*** Get block by number");
    //println!("Request: {:?}", request);

    let number = if request.number == "latest" {
        get_latest_block_number()
    } else {
        match U256::from_str(&request.number) {
            Ok(val) => val,
            Err(err) => return Err(Error::new(format!("{:?}", err))),
        }
    };

    let mut block = match get_block(number) {
        Some(val) => val,
        None => return Ok(BlockResponse { block: None }),
    };

    // if full transactions are requested, attach the TransactionRecord
    if request.full {
        if let Some(val) = StateDb::new().transactions.get(&block.transaction_hash) {
            block.transaction = Some(val);
        }
    }

    let response = BlockResponse { block: Some(block) };
    Ok(response)
}

fn get_transaction_record(request: &TransactionRecordRequest) -> Result<TransactionRecordResponse> {
    info!("*** Get transaction record");
    info!("Hash: {:?}", request.hash);

    let response = TransactionRecordResponse {
        record: StateDb::new().transactions.get(&request.hash),
    };

    Ok(response)
}

fn get_account_balance(request: &AccountRequest) -> Result<AccountBalanceResponse> {
    info!("*** Get account balance");
    info!("Address: {:?}", request.address);

    let balance = get_balance(request.address);
    let response = AccountBalanceResponse { balance: balance };
    Ok(response)
}

fn get_account_nonce(request: &AccountRequest) -> Result<AccountNonceResponse> {
    info!("*** Get account nonce");
    info!("Address: {:?}", request.address);

    let nonce = get_nonce(request.address);
    let response = AccountNonceResponse { nonce: nonce };
    Ok(response)
}

fn get_account_code(request: &AccountRequest) -> Result<AccountCodeResponse> {
    info!("*** Get account code");
    info!("Address: {:?}", request.address);

    let code = get_code_string(request.address);
    let response = AccountCodeResponse { code: code };
    Ok(response)
}

fn execute_raw_transaction(
    request: &ExecuteRawTransactionRequest,
) -> Result<TransactionHashResponse> {
    info!("*** Execute raw transaction");
    info!("Data: {:?}", request.data);

    let value = match read_hex(&request.data) {
        Ok(val) => val,
        Err(err) => return Err(Error::new(format!("{:?}", err))),
    };
    let hash = H256::from(Keccak256::digest(&value).as_slice());

    let rlp = UntrustedRlp::new(&value);

    let transaction: Transaction = rlp.as_val()?;

    let valid = match to_valid::<ByzantiumPatch>(&transaction) {
        Ok(val) => val,
        Err(err) => return Err(Error::new(format!("{:?}", err))),
    };

    let vm = fire_transaction::<ByzantiumPatch>(&valid, get_latest_block_number());
    update_state_from_vm(&vm);
    let (block_number, block_hash) = mine_block(Some(hash));
    save_transaction_record(hash, block_hash, block_number, 0, valid, &vm);

    let response = TransactionHashResponse { hash: hash };
    Ok(response)
}

fn simulate_transaction(
    request: &ExecuteTransactionRequest,
) -> Result<SimulateTransactionResponse> {
    info!("*** Simulate transaction");
    info!("Transaction: {:?}", request.transaction);

    let valid = match unsigned_to_valid(&request.transaction) {
        Ok(val) => val,
        Err(err) => return Err(Error::new(format!("{:?}", err))),
    };

    let vm = fire_transaction::<ByzantiumPatch>(&valid, get_latest_block_number());

    let response = SimulateTransactionResponse {
        result: to_hex(&vm.out()),
        status: match vm.status() {
            VMStatus::ExitedOk => true,
            _ => false,
        },
        used_gas: vm.used_gas(),
    };

    trace!("*** Result: {:?}", response.result);

    Ok(response)
}

// for debugging and testing: executes an unsigned transaction from a web3 sendTransaction
// attempts to execute the transaction without performing any validation
#[cfg(debug_assertions)]
fn debug_execute_unsigned_transaction(
    request: &ExecuteTransactionRequest,
) -> Result<TransactionHashResponse> {
    info!("*** Execute transaction");
    info!("Transaction: {:?}", request.transaction);

    let valid = match unsigned_to_valid(&request.transaction) {
        Ok(val) => val,
        Err(err) => return Err(Error::new(format!("{:?}", err))),
    };

    let hash = unsigned_transaction_hash(&valid);

    let vm = fire_transaction::<ByzantiumPatch>(&valid, get_latest_block_number());
    update_state_from_vm(&vm);
    let (block_number, block_hash) = mine_block(Some(hash));
    save_transaction_record(hash, block_hash, block_number, 0, valid, &vm);

    let response = TransactionHashResponse { hash: hash };
    Ok(response)
}

#[cfg(not(debug_assertions))]
fn debug_execute_unsigned_transaction(
    request: &ExecuteTransactionRequest,
) -> Result<TransactionHashResponse> {
    Err(Error::new("API available only in debug builds"))
}
