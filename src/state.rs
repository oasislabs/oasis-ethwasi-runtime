extern crate alloc;
extern crate ethereum_types;
extern crate hex;
extern crate sha3;

use ekiden_core::error::{Error, Result};
use ekiden_trusted::db::{database_schema, Database, DatabaseHandle};

use std::{collections::HashMap, sync::Arc};

use ethereum_types::{Address, H256, U256};

use evm_api::{AccountState, Block, TransactionRecord};

use ethcore::{
  self,
  executed::Executed,
  journaldb::{self, overlaydb::OverlayDB},
  kvdb,
  state::backend::Basic as BasicBackend,
  transaction::{Action, SignedTransaction, Transaction, UnverifiedTransaction},
};

// use ethcore::{
//   self,
//   executive::{Executed, Executive, TransactOptions},
//   kvdb::KeyValueDB,
//   machine::EthereumMachine,
//   spec::CommonParams,
//   state::{backend::Basic as BasicBackend, State as EthState},
//   transaction::{Action, SignedTransaction, Transaction},
//   vm,
// };

// use sputnikvm::{AccountChange, AccountPatch, Patch, SeqTransactionVM, Storage, TransactionAction,
//                 VMStatus, ValidTransaction, VM};

use std::rc::Rc;

// Create database schema.
database_schema! {
    pub struct StateDb {
        pub genesis_initialized: bool,
        pub transactions: Map<H256, TransactionRecord>,
        pub latest_block_number: U256,
        pub blocks: Map<U256, Block>,
    }
}

pub struct State {
  db: StateDb,
}

type Backend = BasicBackend<OverlayDB>;
type EthState = ethcore::state::State<Backend>;

pub(crate) fn get_backend() -> Backend {
  BasicBackend(OverlayDB::new(
    Arc::new(State::instance()),
    None, /* col */
  ))
}

pub(crate) fn get_state() -> Result<EthState> {
  Ok(ethcore::state::State::from_existing(
    get_backend(),
    get_latest_block()
      .ok_or(Error::new("Genesis not ininitialized"))?
      .state_root,
    U256::zero(),       /* account_start_nonce */
    Default::default(), /* factories */
  )?)
}

pub fn with_state<R, F: FnOnce(&mut EthState) -> Result<R>>(cb: F) -> Result<(R, H256)> {
  let mut state = get_state()?;

  let ret = cb(&mut state)?;

  state.commit();
  let (state_root, mut db) = state.drop();
  db.0.commit();

  Ok((ret, state_root))
}

impl State {
  fn new() -> Self {
    State { db: StateDb::new() }
  }

  pub fn instance() -> State {
    State::new()
  }
}

pub fn get_account_state(address: Address) -> Result<Option<AccountState>> {
  let state = get_state()?;
  if !state.exists_and_not_null(&address)? {
    return Ok(None);
  }
  Ok(Some(AccountState {
    address: address.clone(),
    nonce: state.nonce(&address)?,
    balance: state.balance(&address)?,
    code: get_code_string_from_state(&state, &address)?,
  }))
}

fn get_code_string_from_state(state: &EthState, address: &Address) -> Result<String> {
  Ok(
    state
      .code(address)?
      .map(|code| hex::encode(code.as_ref()))
      .unwrap_or(String::new()),
  )
}

pub fn get_account_storage(address: Address, key: H256) -> Result<H256> {
  Ok(get_state()?.storage_at(&address, &key)?)
}

// TODO: currently returns 0 for nonexistent accounts
//       specified behavior is different for more recent patches
pub fn get_account_nonce(address: &Address) -> Result<U256> {
  Ok(get_state()?.nonce(&address)?)
}

// TODO: currently returns 0 for nonexistent accounts
//       specified behavior is different for more recent patches
pub fn get_account_balance(address: &Address) -> Result<U256> {
  Ok(get_state()?.balance(&address)?)
}

// returns a hex-encoded string directly from storage to avoid unnecessary conversions
pub fn get_code_string(address: &Address) -> Result<String> {
  Ok(get_code_string_from_state(&get_state()?, address)?)
}

pub fn update_account_state(account: &AccountState) -> Result<()> {
  with_state(|state| {
    state.new_contract(
      &account.address,
      account.balance.clone(),
      account.nonce.clone(),
    );
    if account.code.len() > 0 {
      state
        .init_code(
          &account.address,
          hex::decode(&account.code).map_err(|_| Error::new("Code hex decode error."))?,
        )
        .map_err(|_| {
          Error::new(format!(
            "Could not init code for address {:?}.",
            &account.address
          ))
        })
    } else {
      Ok(())
    }
  }).map(|_| ())
}

// pub fn update_account_storage(address: Address, storage: &Storage) {
//     let storage: HashMap<U256, M256> = storage.clone().into();
//     for (key, val) in storage {
//         self.db.account_storage.insert(&(address, key), &val);
//     }
// }

// pub fn update_account_balance<P: Patch>(address: &Address, amount: U256, sign: Sign) {
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

pub fn get_transaction_record(hash: &H256) -> Option<TransactionRecord> {
  StateDb::new().transactions.get(hash)
}

pub fn get_block_hash(number: U256) -> Option<H256> {
  match StateDb::new().blocks.get(&number) {
    Some(block) => Some(block.hash),
    None => None,
  }
}

pub fn get_block(number: U256) -> Option<Block> {
  let state = StateDb::new();
  state.blocks.get(&number)
}

pub fn get_latest_block() -> Option<Block> {
  get_block(get_latest_block_number())
}

pub fn get_latest_block_number() -> U256 {
  StateDb::new()
    .latest_block_number
    .get()
    .unwrap_or(U256::zero())
}

pub fn save_transaction_record(
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

  StateDb::new().transactions.insert(&hash, &record);
}

impl kvdb::KeyValueDB for State {
  fn get(&self, _col: Option<u32>, key: &[u8]) -> kvdb::Result<Option<kvdb::DBValue>> {
    Ok(
      DatabaseHandle::instance()
        .get(key)
        .map(kvdb::DBValue::from_vec),
    )
  }

  fn get_by_prefix(&self, col: Option<u32>, prefix: &[u8]) -> Option<Box<[u8]>> {
    unimplemented!();
  }

  fn write_buffered(&self, transaction: kvdb::DBTransaction) {
    transaction.ops.iter().for_each(|op| match op {
      &kvdb::DBOp::Insert {
        ref key, ref value, ..
      } => {
        DatabaseHandle::instance().insert(key, value.to_vec().as_slice());
      }
      &kvdb::DBOp::Delete { ref key, .. } => {
        DatabaseHandle::instance().remove(key);
      }
    });
  }

  fn flush(&self) -> kvdb::Result<()> {
    Ok(())
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
