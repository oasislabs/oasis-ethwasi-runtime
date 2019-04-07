use std::{fmt, io::Cursor};

use ethcore::{ids::BlockId, spec::Spec};
use ethereum_types::U256;
use failure::Error;
use jsonrpc_core::{self, ErrorCode, Value};
use runtime_ethereum_api::BlockId as EkidenBlockId;

pub fn gwei_to_wei(gwei: u64) -> U256 {
    U256::from(gwei).saturating_mul(U256::from(1_000_000_000))
}

pub fn load_spec() -> Spec {
    #[cfg(feature = "production-genesis")]
    let spec_json = include_str!("../../resources/genesis/genesis.json");
    #[cfg(not(feature = "production-genesis"))]
    let spec_json = include_str!("../../resources/genesis/genesis_testing.json");
    Spec::load(Cursor::new(spec_json)).unwrap()
}

pub fn from_block_id(id: BlockId) -> EkidenBlockId {
    match id {
        BlockId::Number(number) => EkidenBlockId::Number(number.into()),
        BlockId::Hash(hash) => EkidenBlockId::Hash(hash),
        BlockId::Earliest => EkidenBlockId::Earliest,
        BlockId::Latest => EkidenBlockId::Latest,
    }
}

/// Constructs a JSON-RPC error from a string message, with error code -32603.
pub fn jsonrpc_error(err: Error) -> jsonrpc_core::Error {
    jsonrpc_core::Error {
        code: ErrorCode::InternalError,
        message: format!("{}", err),
        data: None,
    }
}

/// Constructs a JSON-RPC error for a transaction execution error.
/// TODO: format error message
pub fn execution_error<T: fmt::Display>(data: T) -> jsonrpc_core::Error {
    jsonrpc_core::Error {
        code: ErrorCode::ServerError(-32015),
        message: format!(
            "Transaction execution error with cause Error {{ message: \"{}\" }}",
            data
        ),
        data: Some(Value::String(format!("{}", data))),
    }
}
