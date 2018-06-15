use std::{collections::BTreeMap, rc::Rc, str::FromStr, sync::Arc};

use ekiden_core::error::{Error, Result};
use ethcore::{
  self,
  executive::{Executed, Executive, TransactOptions},
  journaldb::{self, overlaydb::OverlayDB},
  kvdb::KeyValueDB,
  machine::EthereumMachine,
  spec::CommonParams,
  state::{backend::Basic as BasicBackend, State as EthState},
  transaction::{Action, SignedTransaction, Transaction},
  vm,
};
use ethereum_types::{Address, H256, U256};

use super::{
  miner::{get_latest_block, get_latest_block_number},
  State,
};

/// as per https://github.com/paritytech/parity/blob/master/ethcore/res/ethereum/byzantium_test.json
macro_rules! evm_params {
  () => {{
    let mut params = CommonParams::default();
    params.maximum_extra_data_size = 0x20;
    params.min_gas_limit = 0x1388.into();
    params.network_id = 0x01;
    params.max_code_size = 24576;
    params.eip98_transition = <u64>::max_value();
    params.gas_limit_bound_divisor = 0x0400.into();
    params.registrar = "0xc6d9d2cd449a754c494264e1809c50e34d64562b".into();
    params
  }};
}

macro_rules! get_backend {
  () => {
    ethcore::state::backend::Basic(journaldb::overlaydb::OverlayDB::new(
      Arc::new(State::instance()),
      None, /* col */
    ));
  };
}

macro_rules! get_state {
  ($state_root:expr) => {
    ethcore::state::State::from_existing(
      get_backend!(),
      $state_root,
      U256::zero(),       /* account_start_nonce */
      Default::default(), /* factories */
    )
  };
  () => {
    ethcore::state::State::new(
      get_backend!(),
      U256::zero(),       /* account_start_nonce */
      Default::default(), /* factories */
    )
  };
}

pub fn with_state<R, F: FnOnce(&mut EthState<BasicBackend<OverlayDB>>) -> Result<R>>(
  cb: F,
) -> Result<(R, H256)> {
  let mut state = get_state!(
    get_latest_block()
      .ok_or(Error::new("Genesis not ininitialized"))?
      .state_root
  )?;

  let ret = cb(&mut state)?;

  state.commit();
  let (state_root, mut db) = state.drop();
  db.0.commit();

  Ok((ret, state_root))
}

pub fn execute_transaction(transaction: &SignedTransaction) -> Result<(Executed, H256)> {
  let env_info = {
    let mut env_info = vm::EnvInfo::default();
    env_info.number = get_latest_block_number().into();
    env_info.gas_limit = U256::max_value();
    env_info
  };
  let machine = EthereumMachine::regular(evm_params!(), BTreeMap::new() /* builtins */);

  with_state(|state| {
    Ok(Executive::new(state, &env_info, &machine)
      .transact(&transaction, TransactOptions::with_no_tracing())?)
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use ethcore::{executive::contract_address, kvdb};
  use hex;

  #[test]
  fn test_exec() {
    let sender = Address::zero();

    let mut state = get_state(None).unwrap();

    state.add_balance(
      &sender,
      &U256::from(18),
      ethcore::state::CleanupMode::NoEmpty,
    );

    state.commit().unwrap();
    let (root, mut db) = state.drop();
    db.0.commit();

    let code = hex::decode("3331600055").unwrap();
    let contract = contract_address(
      vm::CreateContractAddress::FromCodeHash,
      &sender,
      &U256::zero(),
      &code,
    ).0;

    let tx = Transaction {
      action: Action::Create,
      value: U256::from(17),
      data: code,
      gas: 0x98765usize.into(),
      gas_price: U256::zero(),
      nonce: U256::zero(),
    }.fake_sign(sender);

    let root = execute_transaction(&tx).unwrap();

    let new_state = get_state(Some(root)).unwrap();

    assert_eq!(new_state.balance(&sender).unwrap(), U256::from(1));
    assert_eq!(new_state.nonce(&sender).unwrap(), U256::from(1));
    assert_eq!(new_state.balance(&contract).unwrap(), U256::from(17));
    assert_eq!(
      new_state.storage_at(&contract, &H256::new()).unwrap(),
      H256::from(&U256::from(1))
    );
  }
}
