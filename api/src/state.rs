use bigint::{Address, Gas, H256, M256, Sign, U256};
use serde::{Serialize, Deserialize};

use std::collections::HashMap;

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

#[derive(Serialize, Deserialize, Debug)]
pub struct TransactionRecordRequest {
    pub hash: H256,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TransactionRecordResponse {
    pub record: Option<TransactionRecord>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TransactionRecord {
    pub hash: H256,
    pub nonce: U256,
    pub block_hash: H256,
    pub block_number: U256,
    pub index: u32,                         // txn index in block, always 0 for single-txn blocks
    pub is_create: bool,                    // is this a create transacation?
    pub from: Option<Address>,              // sender address
    pub to: Option<Address>,                // receiver address, defined if !is_create
    pub gas_used: Gas,                      // gas used to execute this txn
    pub cumulative_gas_used: Gas,           // always equal to gas_used for single-txn blocks
    pub contract_address: Option<Address>,  // address of created contract, defined if is_create
    pub value: U256,
    pub gas_price: Gas,
    pub gas_provided: Gas,
    pub input: String,
    pub status: bool,                       // true for success
    // TODO: add logs
}
