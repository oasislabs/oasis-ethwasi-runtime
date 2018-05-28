use bigint::{Address, Gas, H256, M256, Sign, U256};
use serde::{Serialize, Deserialize};

use std::collections::HashMap;
use generated::api::TransactionRecord;

#[derive(Serialize, Deserialize, Debug)]
pub struct AccountRequest {
    pub address: Address,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AccountBalanceResponse {
    pub balance: U256,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AccountNonceResponse {
    pub nonce: U256,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AccountCodeResponse {
    pub code: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InjectAccountsRequest {
    pub accounts: Vec<AccountState>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AccountState {
    pub nonce: U256,
    pub address: Address,
    pub balance: U256,
    pub storage: HashMap<U256, U256>,
    pub code: String,
}

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
