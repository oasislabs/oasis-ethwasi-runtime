use std::io::Cursor;

use ekiden_core::error::Result;
use ethcore::{
    executive::{contract_address, Executed, Executive, TransactOptions},
    spec::Spec,
    transaction::{LocalizedTransaction, SignedTransaction},
    vm,
};
use ethereum_types::{Address, U256};
use runtime_ethereum_common::{confidential::ConfidentialCtx, BLOCK_GAS_LIMIT};

use super::state::Cache;

lazy_static! {
    pub(crate) static ref GAS_LIMIT: U256 = U256::from(BLOCK_GAS_LIMIT);
    pub(crate) static ref SPEC: Spec = {
        #[cfg(feature = "production-genesis")]
        let spec_json = include_str!("../resources/genesis/genesis.json");
        #[cfg(not(feature = "production-genesis"))]
        let spec_json = include_str!("../resources/genesis/genesis_testing.json");
        Spec::load(Cursor::new(spec_json)).unwrap()
    };
}

fn get_env_info(cache: &Cache) -> vm::EnvInfo {
    let parent = cache.best_block_header();
    let mut env_info = vm::EnvInfo::default();
    env_info.last_hashes = cache.last_hashes(&parent.hash());
    env_info.number = parent.number() + 1;
    env_info.gas_limit = U256::max_value();
    env_info.timestamp = parent.timestamp();
    env_info
}

pub fn simulate_transaction(cache: &Cache, transaction: &SignedTransaction) -> Result<Executed> {
    let mut state = cache.get_state(ConfidentialCtx::new())?;
    let options = TransactOptions::with_no_tracing().dont_check_nonce();
    let exec = Executive::new(&mut state, &get_env_info(cache), SPEC.engine.machine())
        .transact_virtual(&transaction, options)?;
    Ok(exec)
}

// pre-EIP86, contract addresses are calculated using the FromSenderAndNonce scheme
pub fn get_contract_address(sender: &Address, transaction: &LocalizedTransaction) -> Address {
    contract_address(
        SPEC.engine.create_address_scheme(transaction.block_number),
        sender,
        &transaction.nonce,
        &transaction.data,
    )
    .0
}
