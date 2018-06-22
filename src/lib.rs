#![feature(iterator_try_fold)]
#![feature(use_extern_macros)]

extern crate common_types as ethcore_types;
extern crate ekiden_core;
extern crate ekiden_trusted;
extern crate ethcore;
extern crate ethereum_types;
extern crate evm_api;
extern crate hex;
extern crate log;
extern crate protobuf;
extern crate sha3;

mod evm;
#[macro_use]
mod logger;
mod miner;
mod state;
mod util;

use std::str::FromStr;

use ekiden_core::error::{Error, Result};
use ekiden_trusted::{contract::create_contract, enclave::enclave_init};
use ethcore::{rlp,
              transaction::{Action, SignedTransaction, Transaction as EVMTransaction}};
use ethereum_types::{Address, H256, U256};
use evm_api::{error::INVALID_BLOCK_NUMBER, with_api, AccountState, Block, BlockRequestByHash,
              BlockRequestByNumber, FilteredLog, InitStateRequest, LogFilter,
              SimulateTransactionResponse, Transaction, TransactionRecord};

use miner::mine_block;
use state::{block_by_hash, block_by_number, get_latest_block_number, with_state, StateDb};
use util::{from_hex, to_hex};

enclave_init!();

// Create enclave contract interface.
with_api! {
    create_contract!(api);
}

// used for performance debugging
fn debug_null_call(_request: &bool) -> Result<()> {
    Ok(())
}

#[cfg(any(debug_assertions, feature = "benchmark"))]
fn genesis_block_initialized(_request: &bool) -> Result<bool> {
    Ok(StateDb::new().genesis_initialized.is_present())
}

#[cfg(not(any(debug_assertions, feature = "benchmark")))]
fn genesis_block_initialized(_request: &bool) -> Result<bool> {
    Err(Error::new("API available only in debug builds"))
}

// TODO: secure this method so it can't be called by any client.
#[cfg(any(debug_assertions, feature = "benchmark"))]
fn inject_accounts(accounts: &Vec<AccountState>) -> Result<()> {
    let state = StateDb::new();
    if state.genesis_initialized.is_present() {
        return Err(Error::new("Genesis block already created"));
    }

    let (_, root) = with_state(|state| {
        accounts.iter().try_for_each(|ref account| {
            state.new_contract(
                &account.address,
                account.balance.clone(),
                account.nonce.clone(),
            );
            if account.code.len() > 0 {
                state
                    .init_code(&account.address, from_hex(&account.code)?)
                    .map_err(|_| {
                        Error::new(format!(
                            "Could not init code for address {:?}.",
                            &account.address
                        ))
                    })
            } else {
                Ok(())
            }
        })
    })?;

    mine_block(None, root);

    Ok(())
}

#[cfg(not(any(debug_assertions, feature = "benchmark")))]
fn inject_accounts(accounts: &Vec<AccountState>) -> Result<()> {
    Err(Error::new("API available only in debug builds"))
}

// TODO: secure this method so it can't be called by any client.
#[cfg(any(debug_assertions, feature = "benchmark"))]
pub fn inject_account_storage(storages: &Vec<(Address, H256, H256)>) -> Result<()> {
    info!("*** Inject account storage");
    let state = StateDb::new();

    if state.genesis_initialized.is_present() {
        return Err(Error::new("Genesis block already created"));
    }

    let (_, root) = with_state(|state| {
        storages.iter().try_for_each(|&(addr, key, value)| {
            state
                .set_storage(&addr, key.clone(), value.clone())
                .map_err(|_| Error::new("Could not set storage."))
        })
    })?;

    mine_block(None, root);

    Ok(())
}

#[cfg(not(any(debug_assertions, feature = "benchmark")))]
fn inject_account_storage(storage: &Vec<(Address, U256, M256)>) -> Result<()> {
    Err(Error::new("API available only in debug builds"))
}

// TODO: secure this method so it can't be called by any client.
#[cfg(any(debug_assertions, feature = "benchmark"))]
fn init_genesis_block(_block: &InitStateRequest) -> Result<()> {
    info!("*** Init genesis block");
    let state = StateDb::new();

    if state.genesis_initialized.is_present() {
        return Err(Error::new("Genesis block already created"));
    }

    if state::get_latest_block().is_none() {
        mine_block(None, H256::zero());
    }

    state.genesis_initialized.insert(&true);

    Ok(())
}

#[cfg(not(any(debug_assertions, feature = "benchmark")))]
fn init_genesis_block(block: &InitStateRequest) -> Result<()> {
    Err(Error::new("API available only in debug builds"))
}

/// TODO: first argument is ignored; remove once APIs support zero-argument signatures (#246)
pub fn get_block_height(_request: &bool) -> Result<U256> {
    Ok(get_latest_block_number())
}

pub fn get_latest_block_hashes(block_height: &U256) -> Result<Vec<H256>> {
    let mut result = Vec::new();

    let current_block_height = get_latest_block_number();
    let mut next_start = block_height.clone();

    while next_start <= current_block_height {
        let hash = block_by_number(next_start).unwrap().hash;
        result.push(hash);
        next_start = next_start + U256::one();
    }

    Ok(result)
}

fn get_block_by_number(request: &BlockRequestByNumber) -> Result<Option<Block>> {
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

    let mut block = match block_by_number(number) {
        Some(val) => val,
        None => return Ok(None),
    };

    // if full transactions are requested, attach the TransactionRecord
    if request.full {
        if let Some(val) = state::get_transaction_record(&block.transaction_hash) {
            block.transaction = Some(val);
        }
    }

    Ok(Some(block))
}

fn get_block_by_hash(request: &BlockRequestByHash) -> Result<Option<Block>> {
    println!("*** Get block by hash");
    println!("Request: {:?}", request);

    let mut block = match block_by_hash(request.hash) {
        Some(val) => val,
        None => return Ok(None),
    };

    // if full transactions are requested, attach the TransactionRecord
    if request.full {
        if let Some(val) = state::get_transaction_record(&block.transaction_hash) {
            block.transaction = Some(val);
        }
    }

    Ok(Some(block))
}

fn get_logs(filter: &LogFilter) -> Result<Vec<FilteredLog>> {
    info!("*** Get logs");
    info!("Log filter: {:?}", filter);

    util::get_logs_from_filter(filter)
}

pub fn get_transaction_record(hash: &H256) -> Result<Option<TransactionRecord>> {
    info!("*** Get transaction record");
    info!("Hash: {:?}", hash);
    let r = Ok(state::get_transaction_record(hash));
    r
}

pub fn get_account_state(address: &Address) -> Result<Option<AccountState>> {
    info!("*** Get account state");
    info!("Address: {:?}", address);
    state::get_account_state(address)
}

pub fn get_account_balance(address: &Address) -> Result<U256> {
    info!("*** Get account balance");
    info!("Address: {:?}", address);
    state::get_account_balance(address)
}

pub fn get_account_nonce(address: &Address) -> Result<U256> {
    info!("*** Get account nonce");
    info!("Address: {:?}", address);
    state::get_account_nonce(address)
}

pub fn get_account_code(address: &Address) -> Result<String> {
    info!("*** Get account code");
    info!("Address: {:?}", address);
    state::get_code_string(address)
}

pub fn get_storage_at(pair: &(Address, H256)) -> Result<H256> {
    info!("*** Get account storage");
    info!("Address: {:?}", pair);
    state::get_account_storage(pair.0, pair.1)
}

pub fn execute_raw_transaction(request: &String) -> Result<H256> {
    info!("*** Execute raw transaction");
    info!("Data: {:?}", request);
    let tx_rlp = from_hex(request)?;
    let transaction = SignedTransaction::new(rlp::decode(&tx_rlp)?)?;
    info!("Calling transact: {:?}", transaction);
    transact(transaction)
}

fn transact(transaction: SignedTransaction) -> Result<H256> {
    let (exec, state_root) = evm::execute_transaction(&transaction)?;
    info!("transact result: {:?}", exec);
    let tx_hash = transaction.hash();
    let (block_number, block_hash) = mine_block(Some(tx_hash), state_root);
    state::record_transaction(transaction, block_number, block_hash, exec);
    Ok(tx_hash)
}

fn make_unsigned_transaction(request: &Transaction) -> Result<SignedTransaction> {
    let tx = EVMTransaction {
        action: if request.is_call {
            Action::Call(request
                .address
                .ok_or(Error::new("Must provide address for call transaction."))?)
        } else {
            Action::Create
        },
        value: request.value.unwrap_or(U256::zero()),
        data: from_hex(&request.input)?,
        gas: U256::max_value(),
        gas_price: U256::zero(),
        nonce: request.nonce.unwrap_or_else(|| {
            request
                .caller
                .map(|addr| state::get_account_nonce(&addr).unwrap_or(U256::zero()))
                .unwrap_or(U256::zero())
        }),
    };
    Ok(match request.caller {
        Some(addr) => tx.fake_sign(addr),
        None => tx.null_sign(0),
    })
}

pub fn simulate_transaction(request: &Transaction) -> Result<SimulateTransactionResponse> {
    info!("*** Simulate transaction");
    info!("Data: {:?}", request);
    let tx = make_unsigned_transaction(request)?;
    let (exec, _root) = evm::simulate_transaction(&tx)?;
    let result = to_hex(exec.output);
    trace!("*** Result: {:?}", result);
    Ok(SimulateTransactionResponse {
        used_gas: exec.gas_used,
        exited_ok: exec.exception.is_none(),
        result: result,
    })
}

// for debugging and testing: executes an unsigned transaction from a web3 sendTransaction
// attempts to execute the transaction without performing any validation
#[cfg(any(debug_assertions, feature = "benchmark"))]
pub fn debug_execute_unsigned_transaction(request: &Transaction) -> Result<H256> {
    info!("*** Execute transaction");
    info!("Transaction: {:?}", request);
    transact(make_unsigned_transaction(request)?)
}

#[cfg(not(any(debug_assertions, feature = "benchmark")))]
pub fn debug_execute_unsigned_transaction(request: &Transaction) -> Result<H256> {
    Err(Error::new("API available only in debug builds"))
}
