use std::io::Cursor;

use ekiden_core::error::Result;
use ethcore::{executive::{contract_address, Executed, Executive, TransactOptions},
              spec::Spec,
              transaction::{LocalizedTransaction, SignedTransaction},
              vm};
use ethereum_types::{Address, U256};

use super::state::{best_block_header, get_state, last_hashes};
use super::storage::StorageImpl;

lazy_static! {
    pub(crate) static ref SPEC: Spec = {
        #[cfg(not(feature = "benchmark"))]
        let spec_json = include_str!("../resources/genesis/genesis.json");
        #[cfg(feature = "benchmark")]
        let spec_json = include_str!("../resources/genesis/genesis_benchmarking.json");
        Spec::load(Cursor::new(spec_json)).unwrap()
    };
}

fn get_env_info() -> vm::EnvInfo {
    let parent = best_block_header();
    let mut env_info = vm::EnvInfo::default();
    env_info.last_hashes = last_hashes(&parent.hash());
    env_info.number = parent.number() + 1;
    env_info.gas_limit = U256::max_value();
    env_info
}

pub fn simulate_transaction(transaction: &SignedTransaction) -> Result<Executed> {
    let mut state = get_state()?;
    #[cfg(not(feature = "benchmark"))]
    let options = TransactOptions::with_no_tracing();
    #[cfg(feature = "benchmark")]
    let options = TransactOptions::with_no_tracing().dont_check_nonce();
    let mut storage = StorageImpl::new();
    let exec = Executive::new(
        &mut state,
        &get_env_info(),
        SPEC.engine.machine(),
        &mut storage,
    ).transact_virtual(&transaction, options)?;
    Ok(exec)
}

// pre-EIP86, contract addresses are calculated using the FromSenderAndNonce scheme
pub fn get_contract_address(sender: &Address, transaction: &LocalizedTransaction) -> Address {
    contract_address(
        SPEC.engine.create_address_scheme(transaction.block_number),
        sender,
        &transaction.nonce,
        &transaction.data,
    ).0
}
