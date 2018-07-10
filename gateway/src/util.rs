use evm_api::{BlockId as EkidenBlockId, Log};

use ethcore::client::BlockId;
use parity_rpc::v1::types::Log as RpcLog;

pub fn log_to_rpc_log(log: Log) -> RpcLog {
    RpcLog {
        address: log.address.into(),
        topics: log.topics.into_iter().map(Into::into).collect(),
        data: log.data.into(),
        block_hash: Some(log.block_hash.into()),
        block_number: Some(log.block_number.into()),
        transaction_hash: Some(log.transaction_hash.into()),
        transaction_index: Some(log.transaction_index.into()),
        log_index: Some(log.log_index.into()),
        transaction_log_index: Some(log.transaction_log_index.into()),
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
