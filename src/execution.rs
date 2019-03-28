//! Transaction execution.
use std::sync::Arc;

use ethcore::{
    executive::{Executed, Executive, TransactOptions},
    transaction::SignedTransaction,
    vm,
};
use ethereum_types::U256;
use failure::Fallible;
use io_context::Context as IoContext;

use crate::{cache::Cache, genesis};

fn get_env_info(cache: &Cache) -> Fallible<vm::EnvInfo> {
    let parent = cache.best_block_header()?;

    let mut env_info = vm::EnvInfo::default();
    env_info.last_hashes = cache.last_hashes(&parent.hash())?;
    env_info.number = parent.number() + 1;
    env_info.gas_limit = U256::max_value();
    env_info.timestamp = parent.timestamp();
    Ok(env_info)
}

/// Simulate a transaction.
pub fn simulate_transaction(
    ctx: Arc<IoContext>,
    cache: &Cache,
    transaction: &SignedTransaction,
) -> Fallible<Executed> {
    let mut state = cache.get_state(ctx)?;
    let options = TransactOptions::with_no_tracing().dont_check_nonce();
    let exec = Executive::new(
        &mut state,
        &get_env_info(cache)?,
        genesis::SPEC.engine.machine(),
    )
    .transact_virtual(&transaction, options)?;
    Ok(exec)
}
