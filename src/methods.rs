//! Methods exported to Ekiden clients.
use std::sync::Arc;

use ekiden_runtime::{runtime_context, transaction::Context as TxnContext};
use ethcore::{
    error::{Error as EthcoreError, ErrorKind, ExecutionError},
    receipt::Receipt as EthReceipt,
    rlp,
    transaction::{
        Action, SignedTransaction, Transaction as EthcoreTransaction, UnverifiedTransaction,
    },
};
use ethereum_types::{Address, H256, U256};
use failure::{format_err, Fallible};
use io_context::Context as IoContext;
use runtime_ethereum_api::{
    BlockId, ExecuteTransactionResponse, Filter, Log, Receipt, SimulateTransactionResponse,
    Transaction, TransactionRequest,
};
use runtime_ethereum_common::BLOCK_GAS_LIMIT;
use slog::{info, warn};

use crate::{execution, Context};

// TODO: first argument is ignored; remove once APIs support zero-argument signatures (#246)
pub fn get_block_height(_request: &bool, ctx: &mut TxnContext) -> Fallible<U256> {
    let ectx = runtime_context!(ctx, Context);

    Ok(ectx.cache.get_latest_block_number()?.into())
}

pub fn get_block_hash(id: &BlockId, ctx: &mut TxnContext) -> Fallible<Option<H256>> {
    let ectx = runtime_context!(ctx, Context);

    let hash = match *id {
        BlockId::Hash(hash) => Some(hash),
        BlockId::Number(number) => ectx.cache.block_hash(number.into())?,
        BlockId::Earliest => ectx.cache.block_hash(0)?,
        BlockId::Latest => ectx
            .cache
            .block_hash(ectx.cache.get_latest_block_number()?)?,
    };
    Ok(hash)
}

pub fn get_block(id: &BlockId, ctx: &mut TxnContext) -> Fallible<Option<Vec<u8>>> {
    let ectx = runtime_context!(ctx, Context);

    info!(ectx.logger, "get_block"; "id" => ?id);

    let block = match *id {
        BlockId::Hash(hash) => ectx.cache.block_by_hash(hash)?,
        BlockId::Number(number) => ectx.cache.block_by_number(number.into())?,
        BlockId::Earliest => ectx.cache.block_by_number(0)?,
        BlockId::Latest => ectx
            .cache
            .block_by_number(ectx.cache.get_latest_block_number()?)?,
    };

    match block {
        Some(block) => Ok(Some(block.into_inner())),
        None => Ok(None),
    }
}

pub fn get_logs(filter: &Filter, ctx: &mut TxnContext) -> Fallible<Vec<Log>> {
    let ectx = runtime_context!(ctx, Context);

    info!(ectx.logger, "get_logs"; "filter" => ?filter);
    ectx.cache.get_logs(filter)
}

pub fn get_transaction(hash: &H256, ctx: &mut TxnContext) -> Fallible<Option<Transaction>> {
    let ectx = runtime_context!(ctx, Context);

    info!(ectx.logger, "get_transaction"; "hash" => ?hash);
    ectx.cache.get_transaction(hash)
}

pub fn get_receipt(hash: &H256, ctx: &mut TxnContext) -> Fallible<Option<Receipt>> {
    let ectx = runtime_context!(ctx, Context);

    info!(ectx.logger, "get_receipt"; "hash" => ?hash);
    ectx.cache.get_receipt(hash)
}

pub fn get_account_balance(address: &Address, ctx: &mut TxnContext) -> Fallible<U256> {
    let ectx = runtime_context!(ctx, Context);

    info!(ectx.logger, "get_account_balance"; "address" => ?address);
    ectx.cache.get_account_balance(ctx.io_ctx.clone(), address)
}

pub fn get_account_nonce(address: &Address, ctx: &mut TxnContext) -> Fallible<U256> {
    let ectx = runtime_context!(ctx, Context);

    info!(ectx.logger, "get_account_nonce"; "address" => ?address);
    ectx.cache.get_account_nonce(ctx.io_ctx.clone(), address)
}

pub fn get_account_code(address: &Address, ctx: &mut TxnContext) -> Fallible<Option<Vec<u8>>> {
    let ectx = runtime_context!(ctx, Context);

    info!(ectx.logger, "get_account_code"; "address" => ?address);
    ectx.cache.get_account_code(ctx.io_ctx.clone(), address)
}

pub fn get_storage_expiry(address: &Address, ctx: &mut TxnContext) -> Fallible<u64> {
    let ectx = runtime_context!(ctx, Context);

    info!(ectx.logger, "get_storage_expiry"; "address" => ?address);
    ectx.cache.get_storage_expiry(ctx.io_ctx.clone(), address)
}

pub fn get_storage_at(pair: &(Address, H256), ctx: &mut TxnContext) -> Fallible<H256> {
    let ectx = runtime_context!(ctx, Context);

    info!(ectx.logger, "get_storage_at"; "address" => ?pair);
    ectx.cache
        .get_account_storage(ctx.io_ctx.clone(), pair.0, pair.1)
}

pub fn execute_raw_transaction(
    request: &Vec<u8>,
    ctx: &mut TxnContext,
) -> Fallible<ExecuteTransactionResponse> {
    let mut ectx = runtime_context!(ctx, Context);

    info!(ectx.logger, "execute_raw_transaction");

    // Decode the transaction.
    let decoded: UnverifiedTransaction = match rlp::decode(request) {
        Ok(t) => t,
        Err(e) => {
            return Ok(ExecuteTransactionResponse {
                hash: Err(e.to_string()),
                created_contract: false,
                block_gas_limit_reached: false,
                output: Vec::new(),
            });
        }
    };

    // Check that gas < block gas limit.
    if decoded.as_unsigned().gas > U256::from(BLOCK_GAS_LIMIT) {
        return Ok(ExecuteTransactionResponse {
            hash: Err(format!("Requested gas greater than block gas limit.")),
            created_contract: false,
            block_gas_limit_reached: false,
            output: Vec::new(),
        });
    }

    // Check signature.
    let is_create = decoded.as_unsigned().action == Action::Create;
    let signed = match SignedTransaction::new(decoded) {
        Ok(t) => t,
        Err(e) => {
            return Ok(ExecuteTransactionResponse {
                hash: Err(e.to_string()),
                created_contract: false,
                block_gas_limit_reached: false,
                output: Vec::new(),
            });
        }
    };

    // Execute the transaction and handle the result.
    match transact(&mut ectx, signed) {
        Ok(outcome) => Ok(ExecuteTransactionResponse {
            created_contract: is_create,
            hash: Ok(outcome.hash),
            block_gas_limit_reached: false,
            output: outcome.output,
        }),
        Err(EthcoreError(ErrorKind::Execution(ExecutionError::BlockGasLimitReached { .. }), _)) => {
            Ok(ExecuteTransactionResponse {
                created_contract: false,
                hash: Err(format!("block gas limit reached")),
                block_gas_limit_reached: true,
                output: Vec::new(),
            })
        }
        Err(err) => Ok(ExecuteTransactionResponse {
            created_contract: false,
            hash: Err(err.to_string()),
            block_gas_limit_reached: false,
            output: Vec::new(),
        }),
    }
}

struct TransactOutcome {
    /// The receipt for the applied transaction.
    pub receipt: EthReceipt,
    /// The output of the applied transaction.
    pub output: Vec<u8>,
    /// Transaction hash
    pub hash: H256,
}

fn transact(
    ectx: &mut Context,
    transaction: SignedTransaction,
) -> Result<TransactOutcome, EthcoreError> {
    let tx_hash = transaction.hash();
    let outcome = ectx
        .open_block
        .push_transaction_with_outcome(transaction, None, true)?;
    Ok(TransactOutcome {
        receipt: outcome.receipt,
        output: outcome.output,
        hash: tx_hash,
    })
}

fn make_unsigned_transaction(
    io_ctx: Arc<IoContext>,
    ectx: &Context,
    request: &TransactionRequest,
) -> Fallible<SignedTransaction> {
    // this max_gas value comes from
    // https://github.com/oasislabs/parity/blob/ekiden/rpc/src/v1/helpers/fake_sign.rs#L24
    let max_gas = 50_000_000.into();
    let gas = match request.gas {
        Some(gas) if gas > max_gas => {
            warn!(
                ectx.logger,
                "Gas limit capped to {} (from {})", max_gas, gas
            );
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
                    .ok_or(format_err!("Must provide address for call transaction."))?,
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
                .map(|addr| {
                    ectx.cache
                        .get_account_nonce(io_ctx, &addr)
                        .unwrap_or(U256::zero())
                })
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
    ctx: &mut TxnContext,
) -> Fallible<SimulateTransactionResponse> {
    let ectx = runtime_context!(ctx, Context);

    info!(ectx.logger, "simulate_transaction");

    let tx = match make_unsigned_transaction(ctx.io_ctx.clone(), &ectx, request) {
        Ok(t) => t,
        Err(e) => {
            info!(ectx.logger, "simulate_transaction returning error"; "err" => ?e);
            return Ok(SimulateTransactionResponse {
                used_gas: U256::from(0),
                refunded_gas: U256::from(0),
                result: Err(e.to_string()),
            });
        }
    };
    let exec = match execution::simulate_transaction(ctx.io_ctx.clone(), &ectx.cache, &tx) {
        Ok(exec) => exec,
        Err(e) => {
            info!(ectx.logger, "simulate_transaction returning error"; "err" => ?e);
            return Ok(SimulateTransactionResponse {
                used_gas: U256::from(0),
                refunded_gas: U256::from(0),
                result: Err(e.to_string()),
            });
        }
    };

    info!(ectx.logger, "simulate_transaction returning success"; "exec" => ?exec);

    Ok(SimulateTransactionResponse {
        used_gas: exec.gas_used,
        refunded_gas: exec.refunded,
        result: Ok(exec.output),
    })
}

pub fn estimate_gas(request: &TransactionRequest, ctx: &mut TxnContext) -> Fallible<U256> {
    let ectx = runtime_context!(ctx, Context);

    info!(ectx.logger, "estimate_gas");

    let tx = make_unsigned_transaction(ctx.io_ctx.clone(), &ectx, request)?;
    let exec = execution::simulate_transaction(ctx.io_ctx.clone(), &ectx.cache, &tx)?;

    Ok(exec.gas_used + exec.refunded)
}
