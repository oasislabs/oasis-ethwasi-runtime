use std::sync::Arc;

use ekiden_core::error::{Error, Result};
use ekiden_trusted::db::{database_schema, Database, DatabaseHandle};
use ethcore::{
  self,
  executed::Executed,
  journaldb::overlaydb::OverlayDB,
  kvdb,
  state::backend::Basic as BasicBackend,
  transaction::{Action, SignedTransaction},
};
use ethereum_types::{Address, H256, U256};
use evm_api::{AccountState, Block, TransactionRecord};

use super::{
  evm::get_contract_address,
  util::{from_hex, to_hex},
};

// Create database schema.
database_schema! {
    pub struct StateDb {
        pub genesis_initialized: bool,
        pub transactions: Map<H256, TransactionRecord>,
        pub latest_block_number: U256,
        // Key: block number
        pub blocks: Map<U256, Block>,
        // To allow retrieving blocks by hash. Key: block hash, value: block number
        pub block_hashes: Map<H256, U256>,
    }
}

pub struct State {}

type Backend = BasicBackend<OverlayDB>;
type EthState = ethcore::state::State<Backend>;

pub(crate) fn get_backend() -> Backend {
  BasicBackend(OverlayDB::new(
    Arc::new(State::instance()),
    None, /* col */
  ))
}

pub(crate) fn get_state() -> Result<EthState> {
  let backend = get_backend();
  if let Some(block) = get_latest_block() {
    Ok(ethcore::state::State::from_existing(
      backend,
      block.state_root,
      U256::zero(),       /* account_start_nonce */
      Default::default(), /* factories */
    )?)
  } else {
    Ok(ethcore::state::State::new(
      backend,
      U256::zero(),       /* account_start_nonce */
      Default::default(), /* factories */
    ))
  }
}

pub fn with_state<R, F: FnOnce(&mut EthState) -> Result<R>>(cb: F) -> Result<(R, H256)> {
  let mut state = get_state()?;

  let ret = cb(&mut state)?;

  state.commit()?;
  let (state_root, mut db) = state.drop();
  db.0.commit()?;

  Ok((ret, state_root))
}

impl State {
  fn new() -> Self {
    State {}
  }

  pub fn instance() -> State {
    State::new()
  }
}

pub fn get_account_state(address: &Address) -> Result<Option<AccountState>> {
  let state = get_state()?;
  if !state.exists_and_not_null(address)? {
    return Ok(None);
  }
  Ok(Some(AccountState {
    address: address.clone(),
    nonce: state.nonce(address)?,
    balance: state.balance(address)?,
    code: get_code_string_from_state(&state, address)?,
  }))
}

fn get_code_string_from_state(state: &EthState, address: &Address) -> Result<String> {
  Ok(state.code(address)?.map(to_hex).unwrap_or(String::new()))
}

pub fn get_account_storage(address: Address, key: H256) -> Result<H256> {
  Ok(get_state()?.storage_at(&address, &key)?)
}

pub fn get_account_nonce(address: &Address) -> Result<U256> {
  Ok(get_state()?.nonce(&address)?)
}

pub fn get_account_balance(address: &Address) -> Result<U256> {
  Ok(get_state()?.balance(&address)?)
}

// returns a hex-encoded string directly from storage to avoid unnecessary conversions
pub fn get_code_string(address: &Address) -> Result<String> {
  Ok(get_code_string_from_state(&get_state()?, address)?)
}

pub fn update_account_state(account: &AccountState) -> Result<H256> {
  with_state(|state| {
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
  }).map(|(_, root)| root)
}

pub fn set_block(block_number: &U256, block: &Block) {
  let state = StateDb::new();
  state.latest_block_number.insert(block_number);
  state.blocks.insert(block_number, block);
  state.block_hashes.insert(&block.hash, block_number);
}

pub fn record_transaction(
  transaction: SignedTransaction,
  block_number: U256,
  block_hash: H256,
  exec: Executed,
) {
  StateDb::new().transactions.insert(
    &transaction.hash(),
    &TransactionRecord {
      hash: transaction.hash(),
      nonce: transaction.nonce,
      block_hash: block_hash,
      block_number: block_number,
      index: 0,
      is_create: transaction.action == Action::Create,
      from: transaction.sender(),
      to: match transaction.action {
        Action::Create => None,
        Action::Call(address) => Some(address),
      },
      gas_used: exec.gas_used,
      cumulative_gas_used: exec.cumulative_gas_used,
      contract_address: match transaction.action {
        Action::Create => Some(get_contract_address(&transaction)),
        Action::Call(_) => None,
      },
      value: transaction.value,
      gas_price: transaction.gas_price,
      gas_provided: transaction.gas,
      input: to_hex(&transaction.data),
      exited_ok: exec.exception.is_none(),
      logs: exec.logs,
    },
  );
}

pub fn get_transaction_record(hash: &H256) -> Option<TransactionRecord> {
  StateDb::new().transactions.get(hash)
}

pub fn get_block_hash(number: U256) -> Option<H256> {
  match StateDb::new().blocks.get(&number) {
    Some(block) => Some(block.hash),
    None => None,
  }
}

pub fn block_by_number(number: U256) -> Option<Block> {
  let state = StateDb::new();
  state.blocks.get(&number)
}

pub fn block_by_hash(hash: H256) -> Option<Block> {
    match StateDb::new().block_hashes.get(&hash) {
        Some(number) => block_by_number(number),
        None => None,
    }
}

pub fn get_latest_block() -> Option<Block> {
  block_by_number(get_latest_block_number())
}

pub fn get_latest_block_number() -> U256 {
  StateDb::new()
    .latest_block_number
    .get()
    .unwrap_or(U256::zero())
}

impl kvdb::KeyValueDB for State {
  fn get(&self, _col: Option<u32>, key: &[u8]) -> kvdb::Result<Option<kvdb::DBValue>> {
    Ok(
      DatabaseHandle::instance()
        .get(key)
        .map(kvdb::DBValue::from_vec),
    )
  }

  fn get_by_prefix(&self, _col: Option<u32>, _prefix: &[u8]) -> Option<Box<[u8]>> {
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

  fn iter<'a>(&'a self, _col: Option<u32>) -> Box<Iterator<Item = (Box<[u8]>, Box<[u8]>)> + 'a> {
    unimplemented!();
  }

  fn iter_from_prefix<'a>(
    &'a self,
    _col: Option<u32>,
    _prefix: &'a [u8],
  ) -> Box<Iterator<Item = (Box<[u8]>, Box<[u8]>)> + 'a> {
    unimplemented!();
  }

  fn restore(&self, _new_db: &str) -> kvdb::Result<()> {
    unimplemented!();
  }
}
