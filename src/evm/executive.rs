use std::{collections::BTreeMap, rc::Rc, str::FromStr, sync::Arc};

use ekiden_core::error::{Error, Result};
use ethcore::{
  self,
  executive::{Executive, TransactOptions},
  journaldb,
  machine::EthereumMachine,
  spec::CommonParams,
  transaction::{Action, SignedTransaction, Transaction},
  vm,
};
use ethereum_types::{Address, H256, U256};

use super::EthState;

/// as per https://github.com/paritytech/parity/blob/master/ethcore/res/ethereum/byzantium_test.json
pub fn get_evm_params() -> CommonParams {
  let mut params = CommonParams::default();
  params.maximum_extra_data_size = 0x20;
  params.min_gas_limit = 0x1388.into();
  params.network_id = 0x01;
  params.max_code_size = 24576;
  params.eip98_transition = <u64>::max_value();
  params.gas_limit_bound_divisor = 0x0400.into();
  params.registrar = "0xc6d9d2cd449a754c494264e1809c50e34d64562b".into();
  params
}

pub fn execute_transaction(transaction: &SignedTransaction, block_number: U256) -> Result<()> {
  let state = EthState::instance();

  let mut last_hashes = Vec::new();
  last_hashes.resize(256, H256::default());
  for i in 0..255 {
    match state.get_block_hash(block_number - i) {
      Some(hash) => last_hashes[i] = hash,
      None => break,
    }
  }

  let env_info = vm::EnvInfo {
    number: block_number.into(),
    author: Address::default(),
    timestamp: 0,
    gas_limit: U256::max_value(),
    last_hashes: Arc::new(last_hashes),
    gas_used: U256::zero(),
    difficulty: 0.into(),
  };
  let options = TransactOptions::with_no_tracing();
  let machine = EthereumMachine::regular(get_evm_params(), BTreeMap::new() /* builtins */);
  let journal_db = journaldb::new(
    Arc::new(state),
    Default::default(), /* algorithm */
    None,               /* col */
  );
  let state_db = ethcore::state_db::StateDB::new(
    journal_db,
    25 * 1024 * 1024, /* cache size as per parity/cli/mod.rs */
  );

  let mut eth_state = ethcore::state::State::new(
    state_db,
    U256::zero(),       /* account_start_nonce */
    Default::default(), /* factories */
  );

  let exec = Executive::new(&mut eth_state, &env_info, &machine).transact(&transaction, options);

  Ok(())

  // let state = EthState::instance();
  //
  // let sender = tx.sender();
  // let nonce = state.get_account_nonce(&sender);
  //
  // if tx.nonce != nonce {
  //   return Err("invalid nonce");
  // }
  //
  // let sched = {
  //   // let mut sched = vm::Schedule::new_constantinople();
  //   let mut sched = vm::Schedule::new_byzantium();
  //   sched.wasm = Some(vm::WasmCosts::default());
  // };
  //
  // let base_gas_required = U256::from(tx.gas_required(&schedule));
  //
  // if tx.gas < base_gas_required {
  //   return Err("not enough base gas");
  // } else if t.gas > BLOCK_GAS_LIMIT {
  //   return Err("too much gas required");
  // }
  //
  // let init_gas = tx.gas - base_gas_required;
  //
  // // TODO: we might need bigints here, or at least check overflows.
  // let balance = self.state.balance(&sender)?;
  // let gas_cost = t.gas.full_mul(t.gas_price);
  // let total_cost = U512::from(t.value) + gas_cost;
  //
  // // avoid unaffordable transactions
  // let balance512 = U512::from(balance);
  // if balance512 < total_cost {
  //   return Err(ExecutionError::NotEnoughCash {
  //     required: total_cost,
  //     got: balance512,
  //   });
  // }
  //
  // let mut substate = Substate::new();
  //
  // // NOTE: there can be no invalid transactions from this point.
  // self.state.inc_nonce(&sender)?;
  //
  // self.state.sub_balance(
  //   &sender,
  //   &U256::from(gas_cost),
  //   &mut substate.to_cleanup_mode(&schedule),
  // )?;
  //
  // let (result, output) = match t.action {
  //   Action::Create => {
  //     let (new_address, code_hash) = contract_address(
  //       self.machine.create_address_scheme(self.info.number),
  //       &sender,
  //       &nonce,
  //       &t.data,
  //     );
  //     let params = ActionParams {
  //       code_address: new_address.clone(),
  //       code_hash: code_hash,
  //       address: new_address,
  //       sender: sender.clone(),
  //       origin: sender.clone(),
  //       gas: init_gas,
  //       gas_price: t.gas_price,
  //       value: ActionValue::Transfer(t.value),
  //       code: Some(Arc::new(t.data.clone())),
  //       data: None,
  //       call_type: CallType::None,
  //       params_type: vm::ParamsType::Embedded,
  //     };
  //     let mut out = if output_from_create {
  //       Some(vec![])
  //     } else {
  //       None
  //     };
  //     (
  //       self.create(params, &mut substate, &mut out, &mut tracer, &mut vm_tracer),
  //       out.unwrap_or_else(Vec::new),
  //     )
  //   }
  //   Action::Call(ref address) => {
  //     let params = ActionParams {
  //       code_address: address.clone(),
  //       address: address.clone(),
  //       sender: sender.clone(),
  //       origin: sender.clone(),
  //       gas: init_gas,
  //       gas_price: t.gas_price,
  //       value: ActionValue::Transfer(t.value),
  //       code: self.state.code(address)?,
  //       code_hash: Some(self.state.code_hash(address)?),
  //       data: Some(t.data.clone()),
  //       call_type: CallType::Call,
  //       params_type: vm::ParamsType::Separate,
  //     };
  //     let mut out = vec![];
  //     (
  //       self.call(
  //         params,
  //         &mut substate,
  //         BytesRef::Flexible(&mut out),
  //         &mut tracer,
  //         &mut vm_tracer,
  //       ),
  //       out,
  //     )
  //   }
  // };
  //
  // // finalize here!
  // Ok(self.finalize(
  //   t,
  //   substate,
  //   result,
  //   output,
  //   tracer.drain(),
  //   vm_tracer.drain(),
  // )?)
}
