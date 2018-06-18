#![feature(iterator_try_fold)]
#![feature(use_extern_macros)]

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

use std::str::FromStr;

use ekiden_core::error::{Error, Result};
use ekiden_trusted::{contract::create_contract, enclave::enclave_init};
use ethcore::{
  executed::Executed,
  rlp,
  transaction::{Action, SignedTransaction, Transaction as EVMTransaction},
};
use ethereum_types::{Address, H256, U256};
use evm_api::{
  error::INVALID_BLOCK_NUMBER, with_api, AccountState, Block, BlockRequest, InitStateRequest,
  SimulateTransactionResponse, Transaction, TransactionRecord,
};
use sha3::{Digest, Keccak256};

use miner::mine_block;
use state::{get_block, get_latest_block_number, with_state, StateDb};

enclave_init!();

// Create enclave contract interface.
with_api! {
    create_contract!(api);
}

#[cfg(debug_assertions)]
pub fn genesis_block_initialized(_request: &bool) -> Result<bool> {
  Ok(StateDb::new().genesis_initialized.is_present())
}

#[cfg(not(debug_assertions))]
pub fn genesis_block_initialized(_request: &bool) -> Result<bool> {
  Err(Error::new("API available only in debug builds"))
}

// TODO: secure this method so it can't be called by any client.
#[cfg(debug_assertions)]
pub fn inject_accounts(accounts: &Vec<AccountState>) -> Result<()> {
  if StateDb::new().genesis_initialized.is_present() {
    return Err(Error::new("Genesis block already created"));
  }

  accounts.iter().try_for_each(state::update_account_state)
}

// TODO: secure this method so it can't be called by any client.
#[cfg(debug_assertions)]
pub fn inject_account_storage(storages: &Vec<(Address, H256, H256)>) -> Result<()> {
  let state = StateDb::new();

  if state.genesis_initialized.is_present() {
    return Err(Error::new("Genesis block already created"));
  }

  with_state(|state| {
    storages.iter().try_for_each(|&(addr, key, value)| {
      state
        .set_storage(&addr, key.clone(), value.clone())
        .map_err(|_| Error::new("Could not set storage."))
    })
  }).map(|_| ())
}

// TODO: secure this method so it can't be called by any client.
#[cfg(debug_assertions)]
pub fn init_genesis_block(block: &InitStateRequest) -> Result<()> {
  info!("*** Init genesis block");
  let state = StateDb::new();

  if state.genesis_initialized.is_present() {
    return Err(Error::new("Genesis block already created"));
  }

  // Mine block 0 with no transactions
  mine_block(None, block.state_root);

  state.genesis_initialized.insert(&true);
  Ok(())
}

#[cfg(not(debug_assertions))]
pub fn init_genesis_block(block: &InitStateRequest) -> Result<()> {
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
    let hash = get_block(next_start).unwrap().hash;
    result.push(hash);
    next_start = next_start + U256::one();
  }

  Ok(result)
}

pub fn get_block_by_number(request: &BlockRequest) -> Result<Option<Block>> {
  println!("*** Get block by number");
  println!("Request: {:?}", request);

  let number = if request.number == "latest" {
    get_latest_block_number()
  } else {
    match U256::from_str(&request.number) {
      Ok(val) => val,
      Err(_err) => return Err(Error::new(INVALID_BLOCK_NUMBER)),
    }
  };

  let mut block = match get_block(number) {
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

pub fn get_transaction_record(hash: &H256) -> Result<Option<TransactionRecord>> {
  info!("*** Get transaction record");
  info!("Hash: {:?}", hash);

  Ok(state::get_transaction_record(hash))
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
  state::get_account_storage(pair.0, pair.1)
}

pub fn execute_raw_transaction(request: &String) -> Result<H256> {
  info!("*** Execute raw transaction");
  info!("Data: {:?}", request);
  let tx_rlp = hex::decode(request)?;
  let tx_hash = H256::from(Keccak256::digest(&tx_rlp).as_slice());
  let transaction = SignedTransaction::new(rlp::decode(&tx_rlp)?)?;
  let (exec, state_root) = evm::execute_transaction(&transaction)?;
  let (block_number, block_hash) = mine_block(Some(tx_hash), state_root);
  state::record_transaction(transaction, block_number, block_hash, exec);
  Ok(tx_hash)
}

fn do_simulated_transaction(request: &Transaction) -> Result<(Executed, H256)> {
  let tx = EVMTransaction {
    action: if request.is_call {
      Action::Call(request
        .address
        .ok_or(Error::new("Must provide address for call transaction."))?)
    } else {
      Action::Create
    },
    value: request.value.unwrap_or(U256::zero()),
    data: hex::decode(&request.input)?,
    gas: U256::max_value(),
    gas_price: U256::zero(),
    nonce: request.nonce.unwrap_or(U256::zero()),
  };
  let tx = match request.caller {
    Some(addr) => tx.fake_sign(addr),
    None => tx.null_sign(0),
  };
  Ok((evm::simulate_transaction(&tx)?, tx.hash()))
}

pub fn simulate_transaction(request: &Transaction) -> Result<SimulateTransactionResponse> {
  let exec = do_simulated_transaction(request)?.0;
  let result = hex::encode(exec.output);
  trace!("*** Result: {:?}", result);
  Ok(SimulateTransactionResponse {
    used_gas: exec.gas_used,
    exited_ok: exec.exception.is_none(),
    result: result,
  })
}

// for debugging and testing: executes an unsigned transaction from a web3 sendTransaction
// attempts to execute the transaction without performing any validation
#[cfg(debug_assertions)]
pub fn debug_execute_unsigned_transaction(request: &Transaction) -> Result<H256> {
  info!("*** Execute transaction");
  info!("Transaction: {:?}", request);

  do_simulated_transaction(request).map(|(_exec, hash)| hash)
}

#[cfg(not(debug_assertions))]
pub fn debug_execute_unsigned_transaction(request: &Transaction) -> Result<H256> {
  Err(Error::new("API available only in debug builds"))
}
