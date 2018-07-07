use ethereum_types::{Address, H256, U256};

use ethcore_types::log_entry::LogEntry;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FilteredLog {
    pub removed: bool,
    pub log_index: usize,
    pub transaction_index: usize,
    pub transaction_hash: H256,
    pub block_hash: H256,
    pub block_number: U256,
    pub data: Vec<u8>,
    pub topics: Vec<H256>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AccountState {
    pub nonce: U256,
    pub address: Address,
    pub balance: U256,
    pub code: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Block {
    pub number: U256,
    pub hash: H256,
    pub parent_hash: H256,
    pub transaction_hash: H256,
    pub transaction: Option<TransactionRecord>,
    pub state_root: H256,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TransactionRecord {
    pub hash: H256,
    pub nonce: U256,
    pub block_hash: H256,
    pub block_number: U256,
    pub index: usize,        // txn index in block, always 0 for single-txn blocks
    pub is_create: bool,     // is this a create transacation?
    pub from: Address,       // sender address
    pub to: Option<Address>, // receiver address, defined if !is_create
    pub gas_used: U256,      // gas used to execute this txn
    pub cumulative_gas_used: U256, // always equal to gas_used for single-txn blocks
    pub contract_address: Option<Address>, // address of created contract, defined if is_create
    pub value: U256,
    pub gas_price: U256,
    pub gas_provided: U256,
    pub input: String,
    pub exited_ok: bool, // true for success
    pub logs: Vec<LogEntry>,
}

// An unsigned transaction request.
#[derive(Serialize, Deserialize, Debug)]
pub struct TransactionRequest {
    // The nonce from web3. It's a monotonic counter per account.
    pub nonce: Option<U256>, // optional
    // The "from" addr.
    pub caller: Option<Address>,
    // True if it's a call to a contract, with a "to" addr. If it's not a call, it's a "create."
    pub is_call: bool,
    // The "to" addr for a call to a contract.
    pub address: Option<Address>, // defined if is_call = true
    // Opaque call input.
    pub input: Option<Vec<u8>>,
    pub value: Option<U256>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SimulateTransactionResponse {
    pub used_gas: U256,
    pub exited_ok: bool, // ExitedOk => true
    pub result: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InitStateRequest {}
