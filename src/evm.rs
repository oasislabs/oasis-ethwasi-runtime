use std::{io::Cursor, sync::Arc};

use ekiden_core::error::Result;
use ethcore::{executive::{contract_address, Executed, Executive, TransactOptions},
              machine::EthereumMachine,
              spec::Spec,
              transaction::{LocalizedTransaction, SignedTransaction},
              vm};
use ethereum_types::{Address, U256};

use super::state::{block_hashes_since, get_latest_block_number, get_state, BlockOffset};

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
    let mut env_info = vm::EnvInfo::default();
    env_info.last_hashes = Arc::new(block_hashes_since(BlockOffset::Offset(256)));
    env_info.number = get_latest_block_number() + 1;
    env_info.gas_limit = U256::max_value();
    env_info
}

pub fn simulate_transaction(transaction: &SignedTransaction) -> Result<Executed> {
    let mut state = get_state()?;
    #[cfg(not(feature = "benchmark"))]
    let options = TransactOptions::with_no_tracing();
    #[cfg(feature = "benchmark")]
    let options = TransactOptions::with_no_tracing().dont_check_nonce();
    let exec = Executive::new(&mut state, &get_env_info(), SPEC.engine.machine())
        .transact_virtual(&transaction, options)?;
    Ok(exec)
}

// pre-EIP86, contract addresses are calculated using the FromSenderAndNonce scheme
pub fn get_contract_address(transaction: &mut LocalizedTransaction) -> Address {
    contract_address(
        SPEC.engine.create_address_scheme(transaction.block_number),
        &transaction.sender(),
        &transaction.nonce,
        &transaction.data,
    ).0
}
