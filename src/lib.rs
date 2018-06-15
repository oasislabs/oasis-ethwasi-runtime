#![feature(use_extern_macros)]
#![feature(alloc)]

#[macro_use]
mod logger;
mod evm;
mod miner;
mod state;
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

use evm_api::error::INVALID_BLOCK_NUMBER;
use evm_api::{with_api, AccountState, Block, BlockRequest, InitStateRequest,
              SimulateTransactionResponse, Transaction, TransactionRecord};

use sputnikvm::{VMStatus, VM};
//use sputnikvm_network_classic::MainnetEIP160Patch;

use bigint::{Address, H256, M256, U256};
use block::Transaction as BlockTransaction;
use hexutil::{read_hex, to_hex};
use sha3::{Digest, Keccak256};

use std::str::FromStr;

use evm::patch::ByzantiumPatch;
use evm::{fire_transaction, update_state_from_vm};

use state::{EthState, StateDb};

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
fn genesis_block_initialized(_request: &bool) -> Result<bool> {
    Ok(StateDb::new().genesis_initialized.is_present())
}

#[cfg(not(debug_assertions))]
fn genesis_block_initialized(_request: &bool) -> Result<bool> {
    Err(Error::new("API available only in debug builds"))
}

// TODO: secure this method so it can't be called by any client.
#[cfg(debug_assertions)]
fn inject_accounts(accounts: &Vec<AccountState>) -> Result<()> {
    let state = StateDb::new();

    if state.genesis_initialized.is_present() {
        return Err(Error::new("Genesis block already created"));
    }

    // Insert account states
    for account in accounts {
        state.accounts.insert(&account.address, &account);
    }

    Ok(())
}

// TODO: secure this method so it can't be called by any client.
#[cfg(debug_assertions)]
fn inject_account_storage(storage: &Vec<(Address, U256, M256)>) -> Result<()> {
    let state = StateDb::new();

    if state.genesis_initialized.is_present() {
        return Err(Error::new("Genesis block already created"));
    }

    for &(address, index, value) in storage {
        state.account_storage.insert(&(address, index), &value);
    }

    Ok(())
}

// TODO: secure this method so it can't be called by any client.
#[cfg(debug_assertions)]
fn init_genesis_block(_block: &InitStateRequest) -> Result<()> {
    info!("*** Init genesis block");
    let state = StateDb::new();

    if state.genesis_initialized.is_present() {
        return Err(Error::new("Genesis block already created"));
    }

    // Mine block 0 with no transactions
    mine_block(None);

    state.genesis_initialized.insert(&true);
    Ok(())
}

#[cfg(not(debug_assertions))]
fn init_genesis_block(block: &InitStateRequest) -> Result<()> {
    Err(Error::new("API available only in debug builds"))
}

/// TODO: first argument is ignored; remove once APIs support zero-argument signatures (#246)
fn get_block_height(_request: &bool) -> Result<U256> {
    Ok(get_latest_block_number())
}

fn get_latest_block_hashes(block_height: &U256) -> Result<Vec<H256>> {
    let mut result = Vec::new();

    let current_block_height = get_latest_block_number();
    let mut next_start = block_height.clone();

    while next_start <= current_block_height {
        let hash = get_block(next_start).unwrap().hash;
        result.push(hash);
        next_start = next_start + U256::one();
    }

    Ok(result)
}

fn get_block_by_number(request: &BlockRequest) -> Result<Option<Block>> {
    //println!("*** Get block by number");
    //println!("Request: {:?}", request);

    let number = if request.number == "latest" {
        get_latest_block_number()
    } else {
        match U256::from_str(&request.number) {
            Ok(val) => val,
            Err(_) => return Err(Error::new(INVALID_BLOCK_NUMBER)),
        }
    };

    let mut block = match get_block(number) {
        Some(val) => val,
        None => return Ok(None),
    };

    // if full transactions are requested, attach the TransactionRecord
    if request.full {
        if let Some(val) = EthState::instance().get_transaction_record(&block.transaction_hash) {
            block.transaction = Some(val);
        }
    }

    Ok(Some(block))
}

fn get_transaction_record(hash: &H256) -> Result<Option<TransactionRecord>> {
    info!("*** Get transaction record");
    info!("Hash: {:?}", hash);

    Ok(EthState::instance().get_transaction_record(hash))
}

fn get_account_balance(address: &Address) -> Result<U256> {
    info!("*** Get account balance");
    info!("Address: {:?}", address);

    Ok(EthState::instance().get_account_balance(address))
}

fn get_account_nonce(address: &Address) -> Result<U256> {
    info!("*** Get account nonce");
    info!("Address: {:?}", address);

    Ok(EthState::instance().get_account_nonce(address))
}

fn get_account_code(address: &Address) -> Result<String> {
    info!("*** Get account code");
    info!("Address: {:?}", address);

    Ok(EthState::instance().get_code_string(address))
}

fn get_storage_at(pair: &(Address, U256)) -> Result<M256> {
    info!("*** Get storage at");
    let &(address, index) = pair;
    info!("Address: {:?} @ {:?}", address, index);

    Ok(EthState::instance().get_account_storage(address, index))
}

fn execute_raw_transaction(request: &String) -> Result<H256> {
    info!("*** Execute raw transaction");
    info!("Data: {:?}", request);

    let value = match read_hex(request) {
        Ok(val) => val,
        Err(err) => return Err(Error::new(format!("{:?}", err))),
    };
    let hash = H256::from(Keccak256::digest(&value).as_slice());

    let rlp = UntrustedRlp::new(&value);

    let transaction: BlockTransaction = rlp.as_val()?;

    let valid = match to_valid::<ByzantiumPatch>(&transaction) {
        Ok(val) => val,
        Err(err) => return Err(Error::new(format!("{:?}", err))),
    };

    let vm = fire_transaction::<ByzantiumPatch>(&valid, get_latest_block_number());
    update_state_from_vm(&vm);
    let (block_number, block_hash) = mine_block(Some(hash));
    EthState::instance().save_transaction_record(hash, block_hash, block_number, 0, valid, &vm);

    Ok(hash)
}

fn simulate_transaction(request: &Transaction) -> Result<SimulateTransactionResponse> {
    info!("*** Simulate transaction");
    info!("Transaction: {:?}", request);

    let valid = match unsigned_to_valid(&request) {
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
fn debug_execute_unsigned_transaction(request: &Transaction) -> Result<H256> {
    info!("*** Execute transaction");
    info!("Transaction: {:?}", request);

    let valid = match unsigned_to_valid(&request) {
        Ok(val) => val,
        Err(err) => return Err(Error::new(format!("{:?}", err))),
    };

    let hash = unsigned_transaction_hash(&valid);

    let vm = fire_transaction::<ByzantiumPatch>(&valid, get_latest_block_number());
    update_state_from_vm(&vm);
    let (block_number, block_hash) = mine_block(Some(hash));
    EthState::instance().save_transaction_record(hash, block_hash, block_number, 0, valid, &vm);

    Ok(hash)
}

#[cfg(not(debug_assertions))]
fn debug_execute_unsigned_transaction(request: &Transaction) -> Result<H256> {
    Err(Error::new("API available only in debug builds"))
}
