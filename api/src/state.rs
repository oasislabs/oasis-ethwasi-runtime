use bigint::{Address, Gas, H256, M256, Sign, U256};
use serde::{Serialize, Deserialize};

use generated::api::TransactionRecord;

#[derive(Serialize, Deserialize, Debug)]
pub struct Block {
    pub number: U256,
    pub hash: H256,
    pub parent_hash: H256,
    pub transaction_hash: H256,
    pub transaction: Option<TransactionRecord>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BlockRequest {
    pub number: String,
    pub full: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BlockResponse {
    pub block: Option<Block>,
}
