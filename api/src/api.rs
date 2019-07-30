use ekiden_runtime::runtime_api;
use ethereum_types::{Address, Bloom, H256, U256};
use failure::Fail;
use serde_derive::{Deserialize, Serialize};

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
#[derive(Debug, Fail)]
pub enum TransactionError {
    #[fail(display = "block gas limit reached")]
    BlockGasLimitReached,
    #[fail(display = "duplicate transaction")]
    DuplicateTransaction,
    #[fail(display = "insufficient gas price")]
    GasPrice,
    #[fail(display = "requested gas greater than block gas limit")]
    TooMuchGas,
}

/// Name of the method which executes an ethereum transaction.
pub const METHOD_ETH_TXN: &'static str = "ethereum_transaction";

runtime_api! {
    pub fn ethereum_transaction(ByteBuf) -> ExecutionResult;
}
