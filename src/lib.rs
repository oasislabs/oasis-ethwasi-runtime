#![feature(int_to_from_bytes)]

extern crate common_types as ethcore_types;
extern crate ekiden_common;
extern crate ekiden_core;
extern crate ekiden_storage_base;
extern crate ekiden_storage_dummy;
extern crate ekiden_storage_lru;
extern crate ekiden_trusted;
extern crate elastic_array;
extern crate ethcore;
extern crate ethereum_api;
extern crate ethereum_types;
extern crate hashdb;
extern crate hex;
extern crate keccak_hash;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate protobuf;
extern crate runtime_ethereum_common;
extern crate sha3;

extern crate ekiden_keymanager_client;
extern crate ekiden_keymanager_common;

#[cfg(feature = "test")]
extern crate ekiden_roothash_base;
#[cfg(feature = "test")]
extern crate ethkey;

mod evm;
mod state;
#[cfg(feature = "test")]
pub mod test;

use ekiden_keymanager_client::use_key_manager_contract;

use std::sync::Arc;

use ekiden_core::error::{Error, Result};
use ekiden_storage_base::StorageBackend;
#[cfg(not(target_env = "sgx"))]
use ekiden_storage_dummy::DummyStorageBackend;
#[cfg(target_env = "sgx")]
use ekiden_trusted::db::untrusted::UntrustedStorageBackend;
use ekiden_trusted::{
    db::{Database, DatabaseHandle},
    enclave::enclave_init,
    runtime::{
        configure_runtime_dispatch_batch_handler, create_runtime,
        dispatcher::{BatchHandler, RuntimeCallContext},
    },
};
use ethcore::{
    block::{IsBlock, OpenBlock},
    rlp,
    transaction::{
        Action, SignedTransaction, Transaction as EthcoreTransaction, UnverifiedTransaction,
    },
};
use ethereum_api::{
    with_api, BlockId, ExecuteTransactionResponse, Filter, Log, Receipt,
    SimulateTransactionResponse, Transaction, TransactionRequest,
};
use ethereum_types::{Address, H256, U256};

use self::state::Cache;

enclave_init!();

// Create enclave runtime interface.
with_api! {
    create_runtime!(api);
}

/// This path must match the path used to generate the key manager
/// enclave identity. See build.rs.
use_key_manager_contract!("generated/ekiden-key-manager.identity");

/// Ethereum-specific batch context.
pub struct EthereumContext<'a> {
    /// Blockchain cache.
    pub cache: Cache,
    /// Currently open block.
    block: OpenBlock<'a>,
    /// Force emitting a block.
    force_emit_block: bool,
}

impl<'a> EthereumContext<'a> {
    /// Create new Ethereum-specific batch context.
    pub fn new(storage: Arc<StorageBackend>, db: DatabaseHandle) -> Box<Self> {
        let cache = Cache::from_global(storage, db);

        Box::new(EthereumContext {
            block: cache.new_block().unwrap(),
            cache,
            force_emit_block: false,
        })
    }
}

#[cfg(target_env = "sgx")]
pub struct EthereumBatchHandler;
#[cfg(not(target_env = "sgx"))]
pub struct EthereumBatchHandler {
    /// Allow to configure the storage backend in non-SGX environments.
    pub storage: Arc<StorageBackend>,
}

impl BatchHandler for EthereumBatchHandler {
    fn start_batch(&self, ctx: &mut RuntimeCallContext) {
        // Set max log level to Info (default: Trace) to reduce logger OCALLs.
        log::set_max_level(log::LevelFilter::Info);

        // Obtain current root hash from the block header.
        let root_hash = ctx.header.state_root;

        info!("start_batch, root hash: {:?}", root_hash);

        // Create a new storage backend.
        #[cfg(target_env = "sgx")]
        let storage = Arc::new(UntrustedStorageBackend::new());
        #[cfg(not(target_env = "sgx"))]
        let storage = self.storage.clone();

        // Create a fresh database instance for the given root hash.
        let mut db = DatabaseHandle::new(storage.clone());
        db.set_root_hash(root_hash).unwrap();

        let mut ectx = EthereumContext::new(storage, db);
        ectx.block.set_timestamp(ctx.header.timestamp);
        ctx.runtime = ectx;

        info!("runtime context initialized");
    }

    fn end_batch(&self, ctx: RuntimeCallContext) {
        let mut ectx = *ctx.runtime.downcast::<EthereumContext>().unwrap();

        info!("end_batch");

        // Finalize the block if it contains any transactions.
        if !ectx.block.transactions().is_empty() || ectx.force_emit_block {
            ectx.cache.add_block(ectx.block.close_and_lock()).unwrap();
        }

        // Update cached value.
        let root_hash = ectx.cache.commit_global();

        // TODO: Get rid of the global database handle instance.
        DatabaseHandle::instance().set_root_hash(root_hash).unwrap();
    }
}

configure_runtime_dispatch_batch_handler!(EthereumBatchHandler);

/// TODO: first argument is ignored; remove once APIs support zero-argument signatures (#246)
pub fn get_block_height(_request: &bool, ctx: &mut RuntimeCallContext) -> Result<U256> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    Ok(ectx.cache.get_latest_block_number().into())
}

fn get_block_hash(id: &BlockId, ctx: &mut RuntimeCallContext) -> Result<Option<H256>> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    let hash = match *id {
        BlockId::Hash(hash) => Some(hash),
        BlockId::Number(number) => ectx.cache.block_hash(number.into()),
        BlockId::Earliest => ectx.cache.block_hash(0),
        BlockId::Latest => ectx.cache.block_hash(ectx.cache.get_latest_block_number()),
    };
    Ok(hash)
}

fn get_block(id: &BlockId, ctx: &mut RuntimeCallContext) -> Result<Option<Vec<u8>>> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    info!("get_block, id: {:?}", id);

    let block = match *id {
        BlockId::Hash(hash) => ectx.cache.block_by_hash(hash),
        BlockId::Number(number) => ectx.cache.block_by_number(number.into()),
        BlockId::Earliest => ectx.cache.block_by_number(0),
        BlockId::Latest => ectx
            .cache
            .block_by_number(ectx.cache.get_latest_block_number()),
    };

    match block {
        Some(block) => Ok(Some(block.into_inner())),
        None => Ok(None),
    }
}

fn get_logs(filter: &Filter, ctx: &mut RuntimeCallContext) -> Result<Vec<Log>> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    info!("get_logs, filter: {:?}", filter);
    Ok(ectx.cache.get_logs(filter))
}

pub fn get_transaction(hash: &H256, ctx: &mut RuntimeCallContext) -> Result<Option<Transaction>> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    info!("get_transaction, hash: {:?}", hash);
    Ok(ectx.cache.get_transaction(hash))
}

pub fn get_receipt(hash: &H256, ctx: &mut RuntimeCallContext) -> Result<Option<Receipt>> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    info!("get_receipt, hash: {:?}", hash);
    Ok(ectx.cache.get_receipt(hash))
}

pub fn get_account_balance(address: &Address, ctx: &mut RuntimeCallContext) -> Result<U256> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    info!("get_account_balance, address: {:?}", address);
    ectx.cache.get_account_balance(address)
}

pub fn get_account_nonce(address: &Address, ctx: &mut RuntimeCallContext) -> Result<U256> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    info!("get_account_nonce, address: {:?}", address);
    ectx.cache.get_account_nonce(address)
}

pub fn get_account_code(
    address: &Address,
    ctx: &mut RuntimeCallContext,
) -> Result<Option<Vec<u8>>> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    info!("get_account_code, address: {:?}", address);
    ectx.cache.get_account_code(address)
}

pub fn get_storage_at(pair: &(Address, H256), ctx: &mut RuntimeCallContext) -> Result<H256> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    info!("get_storage_at, address: {:?}", pair);
    ectx.cache.get_account_storage(pair.0, pair.1)
}

pub fn execute_raw_transaction(
    request: &Vec<u8>,
    ctx: &mut RuntimeCallContext,
) -> Result<ExecuteTransactionResponse> {
    let mut ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    info!("execute_raw_transaction");

    let decoded: UnverifiedTransaction = match rlp::decode(request) {
        Ok(t) => t,
        Err(e) => {
            return Ok(ExecuteTransactionResponse {
                hash: Err(e.to_string()),
                created_contract: false,
            });
        }
    };

    let is_create = decoded.as_unsigned().action == Action::Create;
    let signed = match SignedTransaction::new(decoded) {
        Ok(t) => t,
        Err(e) => {
            return Ok(ExecuteTransactionResponse {
                hash: Err(e.to_string()),
                created_contract: false,
            });
        }
    };
    let result = transact(&mut ectx, signed).map_err(|e| e.to_string());
    Ok(ExecuteTransactionResponse {
        created_contract: if result.is_err() { false } else { is_create },
        hash: result,
    })
}

fn transact(ectx: &mut EthereumContext, transaction: SignedTransaction) -> Result<H256> {
    let tx_hash = transaction.hash();
    ectx.block.push_transaction(transaction, None)?;
    Ok(tx_hash)
}

fn make_unsigned_transaction(
    cache: &Cache,
    request: &TransactionRequest,
) -> Result<SignedTransaction> {
    // this max_gas value comes from
    // https://github.com/oasislabs/parity/blob/ekiden/rpc/src/v1/helpers/fake_sign.rs#L24
    let max_gas = 50_000_000.into();
    let gas = match request.gas {
        Some(gas) if gas > max_gas => {
            warn!("Gas limit capped to {} (from {})", max_gas, gas);
            max_gas
        }
        Some(gas) => gas,
        None => max_gas,
    };
    let tx = EthcoreTransaction {
        action: if request.is_call {
            Action::Call(
                request
                    .address
                    .ok_or(Error::new("Must provide address for call transaction."))?,
            )
        } else {
            Action::Create
        },
        value: request.value.unwrap_or(U256::zero()),
        data: request.input.clone().unwrap_or(vec![]),
        gas: gas,
        gas_price: U256::zero(),
        nonce: request.nonce.unwrap_or_else(|| {
            request
                .caller
                .map(|addr| cache.get_account_nonce(&addr).unwrap_or(U256::zero()))
                .unwrap_or(U256::zero())
        }),
    };
    Ok(match request.caller {
        Some(addr) => tx.fake_sign(addr),
        None => tx.null_sign(0),
    })
}

pub fn simulate_transaction(
    request: &TransactionRequest,
    ctx: &mut RuntimeCallContext,
) -> Result<SimulateTransactionResponse> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    info!("simulate_transaction");
    let tx = match make_unsigned_transaction(&ectx.cache, request) {
        Ok(t) => t,
        Err(e) => {
            return Ok(SimulateTransactionResponse {
                used_gas: U256::from(0),
                refunded_gas: U256::from(0),
                result: Err(e.to_string()),
            });
        }
    };
    let exec = match evm::simulate_transaction(&ectx.cache, &tx) {
        Ok(exec) => exec,
        Err(e) => {
            return Ok(SimulateTransactionResponse {
                used_gas: U256::from(0),
                refunded_gas: U256::from(0),
                result: Err(e.to_string()),
            });
        }
    };

    Ok(SimulateTransactionResponse {
        used_gas: exec.gas_used,
        refunded_gas: exec.refunded,
        result: Ok(exec.output),
    })
}
