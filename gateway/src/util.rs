use std::io::Cursor;
use std::path::Path;

use ethereum_api::{BlockId as EkidenBlockId, Log};

use ethcore::ids::BlockId;
use ethcore::spec::Spec;
use ethereum_types::U256;
#[cfg(not(feature = "read_state"))]
use parity_rpc::v1::types::Log as RpcLog;

pub fn gwei_to_wei(gwei: u64) -> U256 {
    U256::from(gwei).saturating_mul(U256::from(1_000_000_000))
}

pub fn load_spec() -> Spec {
    #[cfg(not(feature = "benchmark"))]
    let spec_json = include_str!("../../resources/genesis/genesis.json");
    #[cfg(feature = "benchmark")]
    let spec_json = include_str!("../../resources/genesis/genesis_benchmarking.json");
    Spec::load(Cursor::new(spec_json)).unwrap()
}

#[cfg(not(feature = "read_state"))]
pub fn log_to_rpc_log(log: Log) -> RpcLog {
    RpcLog {
        address: log.address.into(),
        topics: log.topics.into_iter().map(Into::into).collect(),
        data: log.data.into(),
        block_hash: log.block_hash.map(Into::into),
        block_number: log.block_number.map(Into::into),
        transaction_hash: log.transaction_hash.map(Into::into),
        transaction_index: log.transaction_index.map(Into::into),
        log_index: log.log_index.map(Into::into),
        transaction_log_index: log.transaction_log_index.map(Into::into),
        log_type: "mined".to_owned(),
    }
}

pub fn from_block_id(id: BlockId) -> EkidenBlockId {
    match id {
        BlockId::Number(number) => EkidenBlockId::Number(number.into()),
        BlockId::Hash(hash) => EkidenBlockId::Hash(hash),
        BlockId::Earliest => EkidenBlockId::Earliest,
        BlockId::Latest => EkidenBlockId::Latest,
    }
}
