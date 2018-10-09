use std::io::Cursor;

use ekiden_core::error::{Error, Result};
use ethcore::{executive::{contract_address, Executed, Executive, TransactOptions},
              spec::Spec,
              transaction::{Action, LocalizedTransaction, SignedTransaction},
              vm};
use ethereum_types::{Address, U256};

use super::state::Cache;
use super::storage::GlobalStorage;
use super::{decrypt_transaction, EthereumContext};

lazy_static! {
    pub(crate) static ref SPEC: Spec = {
        #[cfg(not(feature = "benchmark"))]
        let spec_json = include_str!("../resources/genesis/genesis.json");
        #[cfg(feature = "benchmark")]
        let spec_json = include_str!("../resources/genesis/genesis_benchmarking.json");
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
    let mut state = cache.get_state()?;
    #[cfg(not(feature = "benchmark"))]
    let options = TransactOptions::with_no_tracing();
    #[cfg(feature = "benchmark")]
    let options = TransactOptions::with_no_tracing().dont_check_nonce();
    let mut storage = GlobalStorage::new();
    let exec = Executive::new(
        &mut state,
        &get_env_info(cache),
        SPEC.engine.machine(),
        &mut storage,
    ).transact_virtual(&transaction, options)?;
    Ok(exec)
}

pub fn simulate_transaction_enc(
    ectx: &mut EthereumContext,
    tx: SignedTransaction,
) -> Result<Executed> {
    match tx.action {
        Action::Call(to_address) => {
            let mut result = Err(Error::new("unable to simulate transaction"));
            ectx.with_encryption(to_address, |ectx| {
                let (transaction_decrypted, _) = decrypt_transaction(&tx)?;
                result = simulate_transaction(&ectx.cache, &transaction_decrypted);
                Ok(())
            });
            result
        }
        _ => Err(Error::new("unable to simulate transaction")),
    }
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
