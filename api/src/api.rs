use ethereum_types::{Address, Bloom, H256, U256};
use oasis_core_runtime::runtime_api;
use serde_derive::{Deserialize, Serialize};
use thiserror::Error;

// used in runtime_api! macro
#[allow(unused_imports)]
use serde_bytes::ByteBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LogEntry {
    /// The address of the contract executing at the point of the `LOG` operation.
    pub address: Address,
    /// The topics associated with the `LOG` operation.
    pub topics: Vec<H256>,
    /// The data associated with the `LOG` operation.
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

/// Transaction execution result.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecutionResult {
    pub cumulative_gas_used: U256,
    pub gas_used: U256,
    pub log_bloom: Bloom,
    pub logs: Vec<LogEntry>,
    pub status_code: u8,
    #[serde(with = "serde_bytes")]
    pub output: Vec<u8>,
}

/// Ethereum transaction error.
#[derive(Debug, Error)]
pub enum TransactionError {
    #[error("block gas limit reached")]
    BlockGasLimitReached,
    #[error("duplicate transaction")]
    DuplicateTransaction,
    #[error("execution failed: {message}")]
    ExecutionFailure { message: String },
    #[error("insufficient gas price")]
    GasPrice,
    #[error("requested gas greater than block gas limit")]
    TooMuchGas,
}

/// Name of the method which executes a transaction.
pub const METHOD_TX: &'static str = "tx";

runtime_api! {
    pub fn tx(ByteBuf) -> ExecutionResult;
}
