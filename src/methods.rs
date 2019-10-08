//! Methods exported to Ekiden clients.
use ekiden_runtime::{
    runtime_context,
    transaction::{dispatcher::CheckOnlySuccess, Context as TxnContext},
};
use ethcore::{
    rlp,
    transaction::{SignedTransaction, UnverifiedTransaction},
    types::receipt::TransactionOutcome,
};
use ethereum_types::U256;
use failure::Fallible;
use oasis_runtime_api::{ExecutionResult, LogEntry, TransactionError};
#[cfg_attr(feature = "test", allow(unused))]
use oasis_runtime_common::{
    genesis, BLOCK_GAS_LIMIT, MIN_GAS_PRICE_GWEI, TAG_ETH_LOG_ADDRESS, TAG_ETH_LOG_TOPICS,
    TAG_ETH_TX_HASH,
};

use crate::block::BlockContext;

/// Check transactions.
pub mod check {
    use super::*;

    /// Check ethereum transaction.
    pub fn ethereum_transaction(tx: &[u8], _ctx: &mut TxnContext) -> Fallible<SignedTransaction> {
        let decoded: UnverifiedTransaction = rlp::decode(tx)?;

        // Check that gas < block gas limit.
        if decoded.as_unsigned().gas > BLOCK_GAS_LIMIT.into() {
            return Err(TransactionError::TooMuchGas.into());
        }

        // Check signature.
        let signed = SignedTransaction::new(decoded)?;

        // Check gas price.
        if signed.gas_price < MIN_GAS_PRICE_GWEI.into() {
            return Err(TransactionError::GasPrice.into());
        }

        Ok(signed)
    }
}

/// Execute transactions.
pub mod execute {
    use super::*;

    /// Execute an Ethereum transaction.
    pub fn ethereum_transaction(tx: &[u8], ctx: &mut TxnContext) -> Fallible<ExecutionResult> {
        // Perform transaction checks.
        let tx = super::check::ethereum_transaction(tx, ctx)?;

        // If this is a check txn request, return success.
        if ctx.check_only {
            return Err(CheckOnlySuccess::default().into());
        }

        let ectx = runtime_context!(ctx, BlockContext);

        // Check if current block already contains the transaction. Reject if so.
        let tx_hash = tx.hash();
        if ectx.transaction_set.contains(&tx_hash) {
            return Err(TransactionError::DuplicateTransaction.into());
        }

        // Check whether the transaction fits in the current block. If not, return
        // an error indicating that the client should retry.
        let gas_remaining = U256::from(BLOCK_GAS_LIMIT) - ectx.env_info.gas_used;
        if tx.gas > gas_remaining {
            return Err(TransactionError::BlockGasLimitReached.into());
        }

        // Create Ethereum state instance and apply the transaction.
        let outcome = ectx
            .state
            .apply(
                &ectx.env_info,
                genesis::SPEC.engine.machine(),
                &tx,
                false, /* tracing */
                true,  /* should_return_value */
            )
            .map_err(|err| TransactionError::ExecutionFailure {
                message: format!("{}", err),
            })?;

        // Add to set of executed transactions.
        ectx.transaction_set.insert(tx_hash);

        // Calculate the amount of gas used by this transaction and update the
        // cumulative gas used for the batch. Note: receipt.gas_used is the cumulative
        // gas used after executing the given transaction.
        let gas_used = outcome.receipt.gas_used - ectx.env_info.gas_used;
        ectx.env_info.gas_used = outcome.receipt.gas_used;

        // Emit the Ekiden transaction hash so that we can query it.
        #[cfg(not(feature = "test"))]
        {
            ctx.emit_txn_tag(TAG_ETH_TX_HASH, tx_hash);
            for log in &outcome.receipt.logs {
                ctx.emit_txn_tag(TAG_ETH_LOG_ADDRESS, log.address);
                log.topics
                    .iter()
                    .zip(TAG_ETH_LOG_TOPICS.iter())
                    .take(4)
                    .for_each(|(topic, tag)| {
                        ctx.emit_txn_tag(tag, topic);
                    })
            }
        }

        Ok(ExecutionResult {
            cumulative_gas_used: outcome.receipt.gas_used,
            gas_used,
            log_bloom: outcome.receipt.log_bloom,
            logs: outcome
                .receipt
                .logs
                .into_iter()
                .map(|log| LogEntry {
                    address: log.address,
                    topics: log.topics,
                    data: log.data,
                })
                .collect(),
            status_code: match outcome.receipt.outcome {
                TransactionOutcome::StatusCode(code) => code,
                _ => unreachable!("we always use EIP-658 semantics"),
            },
            output: outcome.output,
        })
    }
}
