use ethereum_api::{BlockId as EkidenBlockId, Log};

use ethcore::client::BlockId;
use parity_rpc::v1::types::Log as RpcLog;

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
