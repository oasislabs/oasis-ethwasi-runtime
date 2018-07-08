use ethereum_types::{Address, Bloom, H256, H512, U256};

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
pub struct Receipt {
    pub hash: Option<H256>,
    pub index: Option<U256>,
    pub block_hash: Option<H256>,
    pub block_number: Option<U256>,
    pub cumulative_gas_used: U256,
    pub gas_used: Option<U256>,
    pub contract_address: Option<Address>,
    pub logs: Vec<LogEntry>,
    pub state_root: Option<H256>,
    pub logs_bloom: Bloom,
    pub status_code: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Transaction {
    pub hash: H256,
    pub nonce: U256,
    pub block_hash: Option<H256>,
    pub block_number: Option<U256>,
    pub index: Option<U256>,
    pub from: Address,
    pub to: Option<Address>,
    pub value: U256,
    pub gas_price: U256,
    pub gas: U256,
    pub input: Vec<u8>,
    pub creates: Option<Address>,
    pub raw: Vec<u8>,
    pub public_key: Option<H512>,
    pub chain_id: Option<u64>,
    pub standard_v: U256,
    pub v: U256,
    pub r: U256,
    pub s: U256,
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
