use std::io::Cursor;

use ethereum_api::BlockId as EkidenBlockId;

use ethcore::ids::BlockId;
use ethcore::spec::Spec;
use ethereum_types::U256;
use jsonrpc_core::{Error, ErrorCode};

pub fn gwei_to_wei(gwei: u64) -> U256 {
    U256::from(gwei).saturating_mul(U256::from(1_000_000_000))
}

pub fn load_spec() -> Spec {
    #[cfg(not(any(debug_assertions, feature = "benchmark")))]
    let spec_json = include_str!("../../resources/genesis/genesis.json");
    #[cfg(any(debug_assertions, feature = "benchmark"))]
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
pub fn jsonrpc_error(message: String) -> Error {
    Error {
        code: ErrorCode::InternalError,
        message: message,
        data: None,
    }
}
