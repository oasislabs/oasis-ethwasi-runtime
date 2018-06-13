extern crate alloc;
extern crate ethereum_types;
extern crate hex;
extern crate sha3;

use ekiden_trusted::db::{database_schema, Database, DatabaseHandle};

use std::collections::HashMap;

use ethereum_types::{Address, H256, U256};

use evm_api::{AccountState, Block, TransactionRecord};

use ethcore::{
  self,
  executed::Executed,
  kvdb,
  transaction::{Action, SignedTransaction, Transaction, UnverifiedTransaction},
};

// use sputnikvm::{AccountChange, AccountPatch, Patch, SeqTransactionVM, Storage, TransactionAction,
//                 VMStatus, ValidTransaction, VM};

use std::rc::Rc;

// Create database schema.
database_schema! {
    pub struct StateDb {
        pub genesis_initialized: bool,
        pub accounts: Map<Address, AccountState>,
        pub account_storage: Map<(Address, U256 /* index */), U256>,
        pub transactions: Map<H256, TransactionRecord>,
        pub latest_block_number: U256,
        pub blocks: Map<U256, Block>,
    }
}

pub struct EthState {
  db: StateDb,
}

impl EthState {
  fn new() -> Self {
    EthState { db: StateDb::new() }
  }

  pub fn instance() -> EthState {
    EthState::new()
  }

  pub fn get_account_state(&self, address: Address) -> Option<AccountState> {
    self.db.accounts.get(&address)
  }

  pub fn get_account_storage(&self, address: Address, index: U256) -> U256 {
    match self.db.account_storage.get(&(address, index)) {
      Some(val) => val.clone(),
      None => U256::zero(),
    }
  }
  // TODO: currently returns 0 for nonexistent accounts
  //       specified behavior is different for more recent patches
  pub fn get_account_nonce(&self, address: &Address) -> U256 {
    match self.db.accounts.get(address) {
      Some(account) => account.nonce,
      None => U256::zero(),
    }
  }

  // TODO: currently returns 0 for nonexistent accounts
  //       specified behavior is different for more recent patches
  pub fn get_account_balance(&self, address: &Address) -> U256 {
    match self.db.accounts.get(address) {
      Some(account) => account.balance,
      None => U256::zero(),
    }
  }

  // returns a hex-encoded string directly from storage to avoid unnecessary conversions
  pub fn get_code_string(&self, address: &Address) -> String {
    match self.db.accounts.get(address) {
      Some(account) => account.code.to_string(),
      None => String::new(),
    }
  }

  pub fn update_account_state(
    &self,
    nonce: U256,
    address: Address,
    balance: U256,
    code: &Rc<Vec<u8>>,
  ) {
    let account_state = AccountState {
      nonce: nonce,
      address: address,
      balance: balance,
      code: hex::encode(code.as_ref()),
    };
    self.db.accounts.insert(&address, &account_state);
  }

  // pub fn update_account_storage(&self, address: Address, storage: &Storage) {
  //     let storage: HashMap<U256, M256> = storage.clone().into();
  //     for (key, val) in storage {
  //         self.db.account_storage.insert(&(address, key), &val);
  //     }
  // }

  // pub fn update_account_balance<P: Patch>(&self, address: &Address, amount: U256, sign: Sign) {
  //     match self.db.accounts.get(&address) {
  //         Some(mut account) => {
  //             // Found account. Update balance.
  //             account.balance = match sign {
  //                 Sign::Plus => account.balance + amount,
  //                 Sign::Minus => account.balance - amount,
  //                 _ => panic!(),
  //             };
  //             self.db.accounts.insert(&address, &account);
  //         }
  //         None => {
  //             // Account doesn't exist; create it.
  //             assert_eq!(
  //                 sign,
  //                 Sign::Plus,
  //                 "Can't decrease balance of nonexistent account"
  //             );
  //
  //             // EIP-161d forbids creating accounts with empty (nonce, code, balance)
  //             if P::Account::empty_considered_exists() || amount != U256::from(0) {
  //                 let account_state = AccountState {
  //                     nonce: P::Account::initial_nonce(),
  //                     address: address.clone(),
  //                     balance: amount,
  //                     code: String::new(),
  //                 };
  //                 self.db.accounts.insert(&address, &account_state);
  //             }
  //         }
  //     }
  // }

  pub fn get_transaction_record(&self, hash: &H256) -> Option<TransactionRecord> {
    self.db.transactions.get(hash)
  }

  pub fn get_block_hash(&self, number: U256) -> Option<H256> {
    match self.db.blocks.get(&number) {
      Some(block) => Some(block.hash),
      None => None,
    }
  }

  pub fn get_latest_block_number(&self) -> U256 {
    self.db.latest_block_number.get().unwrap_or(U256::zero())
  }

  pub fn save_transaction_record(
    &self,
    hash: H256,
    block_hash: H256,
    block_number: U256,
    index: u32,
    transaction: SignedTransaction,
    execution: Executed,
  ) {
    let mut record = TransactionRecord {
      hash: hash,
      nonce: transaction.nonce,
      block_hash: block_hash,
      block_number: block_number,
      index: index,
      from: Some(transaction.sender()),
      to: match transaction.action {
        Action::Call(address) => Some(address),
        Action::Create => None,
      },
      gas_used: execution.gas_used,
      cumulative_gas_used: execution.gas_used,
      value: transaction.value,
      gas_price: transaction.gas_price,
      // TODO: assuming this is gas limit rather than gas used, need to confirm
      gas_provided: transaction.gas,
      input: hex::encode(&transaction.data.clone()),
      is_create: false,
      contract_address: None,
      exited_ok: false,
      logs: execution.logs.clone(),
    };
    let createp = execution.contracts_created.into_iter().nth(0);
    record.is_create = createp.is_some();
    record.contract_address = createp;

    record.exited_ok = execution.exception.is_none();

    self.db.transactions.insert(&hash, &record);
  }
}

impl kvdb::KeyValueDB for EthState {
  fn get(&self, col: Option<u32>, key: &[u8]) -> kvdb::Result<Option<kvdb::DBValue>> {
    unimplemented!();
  }

  fn get_by_prefix(&self, col: Option<u32>, prefix: &[u8]) -> Option<Box<[u8]>> {
    unimplemented!();
  }

  fn write_buffered(&self, transaction: kvdb::DBTransaction) {
    unimplemented!();
  }

  fn flush(&self) -> kvdb::Result<()> {
    unimplemented!();
  }

  fn iter<'a>(&'a self, col: Option<u32>) -> Box<Iterator<Item = (Box<[u8]>, Box<[u8]>)> + 'a> {
    unimplemented!();
  }

  fn iter_from_prefix<'a>(
    &'a self,
    col: Option<u32>,
    prefix: &'a [u8],
  ) -> Box<Iterator<Item = (Box<[u8]>, Box<[u8]>)> + 'a> {
    unimplemented!();
  }

  fn restore(&self, new_db: &str) -> kvdb::Result<()> {
    unimplemented!();
  }
}
