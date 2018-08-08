use ethereum_types::{Address, Bloom, H256, H512, U256};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BlockId {
    Hash(H256),
    Number(U256),
    Earliest,
    Latest,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Filter {
    pub from_block: BlockId,
    pub to_block: BlockId,
    pub address: Option<Vec<Address>>,
    pub topics: Vec<Option<Vec<H256>>>,
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Log {
    pub address: Address,
    pub topics: Vec<H256>,
    pub data: Vec<u8>,
    pub block_hash: Option<H256>,
    pub block_number: Option<U256>,
    pub transaction_hash: Option<H256>,
    pub transaction_index: Option<U256>,
    pub log_index: Option<U256>,
    pub transaction_log_index: Option<U256>,
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
    pub logs: Vec<Log>,
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
pub struct ExecuteTransactionResponse {
    pub hash: Result<H256, String>,
    pub created_contract: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SimulateTransactionResponse {
    pub result: Result<Vec<u8>, String>,
    pub used_gas: U256,
}
