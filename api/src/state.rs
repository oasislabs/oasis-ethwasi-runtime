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

// An unsigned transaction.
#[derive(Serialize, Deserialize, Debug)]
pub struct Transaction {
    // The nonce from web3. It's a monotonic counter per account.
    pub nonce: Option<U256>,                // optional
    // The "from" addr.
    pub caller: Option<Address>,
    // True if it's a call to a contract, with a "to" addr. If it's not a call, it's a "create."
    pub is_call: bool,
    // The "to" addr for a call to a contract.
    pub address: Option<Address>,   // defined if is_call = true
    // Opaque call input.
    pub input: String,              // (hex)
    pub value: Option<U256>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ExecuteTransactionRequest {
    pub transaction: Transaction,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ExecuteRawTransactionRequest {
    // A signed transaction (using Ethereum's own format).
    pub data: String,               // (hex)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TransactionHashResponse {
    pub hash: H256,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SimulateTransactionResponse {
    pub used_gas: Gas,
    pub status: bool,               // ExitedOk => true
    pub result: String,             // (hex)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Log {
    // TODO: add logs from VM
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InitStateRequest {
    // TODO: unnecessary struct
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InitStateResponse {
    // TODO: unnecessary struct
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InjectAccountsResponse {
    // TODO: unnecessary struct
}
