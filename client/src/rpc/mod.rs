use jsonrpc_core::{self, IoHandler, Params};
use jsonrpc_http_server::*;
use jsonrpc_macros::Trailing;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{self, Value};
use bigint::{U256, H256, M256, H2048, H64, Address, Gas};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::collections::HashMap;
use sputnikvm::Patch;

mod serves;
mod filter;
mod util;
mod serialize;
mod solidity;

use error::Error;
use super::miner::MinerState;
use self::serialize::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Either<T, U> {
    Left(T),
    Right(U),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum RPCTopicFilter {
    Single(Hex<H256>),
    Or(Vec<Hex<H256>>)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RPCLogFilter {
    pub from_block: Option<String>,
    pub to_block: Option<String>,
    pub address: Option<Hex<Address>>,
    pub topics: Option<Vec<Option<RPCTopicFilter>>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RPCLog {
    pub removed: bool,
    pub log_index: Hex<usize>,
    pub transaction_index: Hex<usize>,
    pub transaction_hash: Hex<H256>,
    pub block_hash: Hex<H256>,
    pub block_number: Hex<U256>,
    pub data: Bytes,
    pub topics: Vec<Hex<H256>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RPCReceipt {
    pub transaction_hash: Hex<H256>,
    pub transaction_index: Hex<usize>,
    pub block_hash: Hex<H256>,
    pub block_number: Hex<U256>,
    pub cumulative_gas_used: Hex<Gas>,
    pub gas_used: Hex<Gas>,
    pub contract_address: Option<Hex<Address>>,
    pub logs: Vec<RPCLog>,
    pub root: Hex<H256>,
    pub status: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RPCBlock {
    pub number: Hex<U256>,
    pub hash: Hex<H256>,
    pub parent_hash: Hex<H256>,
    pub nonce: Hex<H64>,
    pub sha3_uncles: Hex<H256>,
    pub logs_bloom: Hex<H2048>,
    pub transactions_root: Hex<H256>,
    pub state_root: Hex<H256>,
    pub receipts_root: Hex<H256>,
    pub miner: Hex<Address>,
    pub difficulty: Hex<U256>,
    pub total_difficulty: Hex<U256>,
    pub extra_data: Bytes,
    pub size: Hex<usize>,
    pub gas_limit: Hex<Gas>,
    pub gas_used: Hex<Gas>,
    pub timestamp: Hex<u64>,
    pub transactions: Either<Vec<Hex<H256>>, Vec<RPCTransaction>>,
    pub uncles: Vec<Hex<H256>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RPCTransaction {
    pub from: Option<Hex<Address>>,
    pub to: Option<Hex<Address>>,
    pub gas: Option<Hex<Gas>>,
    pub gas_price: Option<Hex<Gas>>,
    pub value: Option<Hex<U256>>,
    pub data: Option<Bytes>,
    pub nonce: Option<Hex<U256>>,

    pub hash: Option<Hex<H256>>,
    pub block_hash: Option<Hex<H256>>,
    pub block_number: Option<Hex<U256>>,
    pub transaction_index: Option<Hex<usize>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RPCTrace {
    pub gas: Hex<Gas>,
    pub return_value: Bytes,
    pub struct_logs: Vec<RPCStep>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct RPCTraceConfig {
    #[serde(default)]
    pub disable_memory: bool,
    #[serde(default)]
    pub disable_stack: bool,
    #[serde(default)]
    pub disable_storage: bool,
    #[serde(default)]
    pub breakpoints: Option<RPCBreakpointConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct RPCBreakpointConfig {
    pub source_map: HashMap<Hex<H256>, RPCSourceMapConfig>,
    pub breakpoints: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RPCSourceMapConfig {
    pub source_map: String,
    pub source_list: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RPCBlockTrace {
    pub struct_logs: Vec<RPCStep>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RPCStep {
    pub depth: usize,
    pub error: String,
    pub gas: Hex<Gas>,
    pub gas_cost: Hex<Gas>,
    pub op: u8,
    pub pc: usize,
    pub opcode_pc: usize,
    pub code_hash: Hex<H256>,
    pub address: Hex<Address>,
    pub breakpoint_index: Option<usize>,
    pub breakpoint: Option<String>,
    pub memory: Option<Vec<Bytes>>,
    pub stack: Option<Vec<Hex<M256>>>,
    pub storage: Option<HashMap<Hex<U256>, Hex<M256>>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RPCDump {
    pub accounts: HashMap<Hex<Address>, RPCDumpAccount>,
    pub root: Hex<H256>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RPCDumpAccount {
    pub balance: Hex<U256>,
    pub code: Bytes,
    pub code_hash: Hex<H256>,
    pub nonce: Hex<U256>,
    pub root: Hex<H256>,
    pub storage: HashMap<Hex<U256>, Hex<M256>>,
}

build_rpc_trait! {
    pub trait EthereumRPC {
        #[rpc(name = "web3_clientVersion")]
        fn client_version(&self) -> Result<String, Error>;
        #[rpc(name = "web3_sha3")]
        fn sha3(&self, Bytes) -> Result<Hex<H256>, Error>;

        #[rpc(name = "net_version")]
        fn network_id(&self) -> Result<String, Error>;
        #[rpc(name = "net_listening")]
        fn is_listening(&self) -> Result<bool, Error>;
        #[rpc(name = "net_peerCount")]
        fn peer_count(&self) -> Result<Hex<usize>, Error>;

        #[rpc(name = "eth_protocolVersion")]
        fn protocol_version(&self) -> Result<String, Error>;
        #[rpc(name = "eth_syncing")]
        fn is_syncing(&self) -> Result<bool, Error>;
        #[rpc(name = "eth_coinbase")]
        fn coinbase(&self) -> Result<Hex<Address>, Error>;
        #[rpc(name = "eth_mining")]
        fn is_mining(&self) -> Result<bool, Error>;
        #[rpc(name = "eth_hashrate")]
        fn hashrate(&self) -> Result<String, Error>;
        #[rpc(name = "eth_gasPrice")]
        fn gas_price(&self) -> Result<Hex<Gas>, Error>;
        #[rpc(name = "eth_accounts")]
        fn accounts(&self) -> Result<Vec<Hex<Address>>, Error>;
        #[rpc(name = "eth_blockNumber")]
        fn block_number(&self) -> Result<Hex<usize>, Error>;
        #[rpc(name = "eth_getBalance")]
        fn balance(&self, Hex<Address>, Trailing<String>) -> Result<Hex<U256>, Error>;
        #[rpc(name = "eth_getStorageAt")]
        fn storage_at(&self, Hex<Address>, Hex<U256>, Trailing<String>) -> Result<Hex<M256>, Error>;
        #[rpc(name = "eth_getTransactionCount")]
        fn transaction_count(&self, Hex<Address>, Trailing<String>) -> Result<Hex<U256>, Error>;
        #[rpc(name = "eth_getBlockTransactionCountByHash")]
        fn block_transaction_count_by_hash(&self, Hex<H256>) -> Result<Option<Hex<usize>>, Error>;
        #[rpc(name = "eth_getBlockTransactionCountByNumber")]
        fn block_transaction_count_by_number(&self, String) -> Result<Option<Hex<usize>>, Error>;
        #[rpc(name = "eth_getUncleCountByBlockHash")]
        fn block_uncles_count_by_hash(&self, Hex<H256>) -> Result<Option<Hex<usize>>, Error>;
        #[rpc(name = "eth_getUncleCountByBlockNumber")]
        fn block_uncles_count_by_number(&self, String) -> Result<Option<Hex<usize>>, Error>;
        #[rpc(name = "eth_getCode")]
        fn code(&self, Hex<Address>, Trailing<String>) -> Result<Bytes, Error>;
        #[rpc(name = "eth_sign")]
        fn sign(&self, Hex<Address>, Bytes) -> Result<Bytes, Error>;
        #[rpc(name = "eth_sendTransaction")]
        fn send_transaction(&self, RPCTransaction) -> Result<Hex<H256>, Error>;
        #[rpc(name = "eth_sendRawTransaction")]
        fn send_raw_transaction(&self, Bytes) -> Result<Hex<H256>, Error>;

        #[rpc(name = "eth_call")]
        fn call(&self, RPCTransaction, Trailing<String>) -> Result<Bytes, Error>;
        #[rpc(name = "eth_estimateGas")]
        fn estimate_gas(&self, RPCTransaction, Trailing<String>) -> Result<Hex<Gas>, Error>;

        #[rpc(name = "eth_getBlockByHash")]
        fn block_by_hash(&self, Hex<H256>, bool) -> Result<Option<RPCBlock>, Error>;
        #[rpc(name = "eth_getBlockByNumber")]
        fn block_by_number(&self, String, bool) -> Result<Option<RPCBlock>, Error>;
        #[rpc(name = "eth_getTransactionByHash")]
        fn transaction_by_hash(&self, Hex<H256>) -> Result<Option<RPCTransaction>, Error>;
        #[rpc(name = "eth_getTransactionByBlockHashAndIndex")]
        fn transaction_by_block_hash_and_index(&self, Hex<H256>, Hex<U256>) -> Result<Option<RPCTransaction>, Error>;
        #[rpc(name = "eth_getTransactionByBlockNumberAndIndex")]
        fn transaction_by_block_number_and_index(&self, String, Hex<U256>) -> Result<Option<RPCTransaction>, Error>;
        #[rpc(name = "eth_getTransactionReceipt")]
        fn transaction_receipt(&self, Hex<H256>) -> Result<Option<RPCReceipt>, Error>;
        #[rpc(name = "eth_getUncleByBlockHashAndIndex")]
        fn uncle_by_block_hash_and_index(&self, Hex<H256>, Hex<U256>) -> Result<Option<RPCBlock>, Error>;
        #[rpc(name = "eth_getUncleByBlockNumberAndIndex")]
        fn uncle_by_block_number_and_index(&self, String, Hex<U256>) -> Result<Option<RPCBlock>, Error>;

        #[rpc(name = "eth_getCompilers")]
        fn compilers(&self) -> Result<Vec<String>, Error>;

        #[rpc(name = "eth_getLogs")]
        fn logs(&self, RPCLogFilter) -> Result<Vec<RPCLog>, Error>;
    }
}

build_rpc_trait! {
    pub trait FilterRPC {
        #[rpc(name = "eth_newFilter")]
        fn new_filter(&self, RPCLogFilter) -> Result<String, Error>;
        #[rpc(name = "eth_newBlockFilter")]
        fn new_block_filter(&self) -> Result<String, Error>;
        #[rpc(name = "eth_newPendingTransactionFilter")]
        fn new_pending_transaction_filter(&self) -> Result<String, Error>;
        #[rpc(name = "eth_uninstallFilter")]
        fn uninstall_filter(&self, String) -> Result<bool, Error>;

        #[rpc(name = "eth_getFilterChanges")]
        fn filter_changes(&self, String) -> Result<Either<Vec<String>, Vec<RPCLog>>, Error>;
        #[rpc(name = "eth_getFilterLogs")]
        fn filter_logs(&self, String) -> Result<Vec<RPCLog>, Error>;
    }
}

build_rpc_trait! {
    pub trait DebugRPC {
        #[rpc(name = "debug_getBlockRlp")]
        fn block_rlp(&self, usize) -> Result<Bytes, Error>;
        #[rpc(name = "debug_traceTransaction")]
        fn trace_transaction(&self, Hex<H256>, Trailing<RPCTraceConfig>)
                             -> Result<RPCTrace, Error>;
        #[rpc(name = "debug_traceBlock")]
        fn trace_block(&self, Bytes, Trailing<RPCTraceConfig>)
                       -> Result<RPCBlockTrace, Error>;
        #[rpc(name = "debug_traceBlockByNumber")]
        fn trace_block_by_number(&self, usize, Trailing<RPCTraceConfig>)
                                 -> Result<RPCBlockTrace, Error>;
        #[rpc(name = "debug_traceBlockByHash")]
        fn trace_block_by_hash(&self, Hex<H256>, Trailing<RPCTraceConfig>)
                               -> Result<RPCBlockTrace, Error>;
        #[rpc(name = "debug_traceBlockFromFile")]
        fn trace_block_from_file(&self, String, Trailing<RPCTraceConfig>)
                                 -> Result<RPCBlockTrace, Error>;
        #[rpc(name = "debug_dumpBlock")]
        fn dump_block(&self, usize) -> Result<RPCDump, Error>;
    }
}

pub fn rpc_loop<P: 'static + Patch + Send>(
    state: Arc<Mutex<MinerState>>, addr: &SocketAddr, channel: Sender<bool>
) {
    let rpc = serves::MinerEthereumRPC::<P>::new(state.clone(), channel);
    let filter = serves::MinerFilterRPC::<P>::new(state.clone());
    let debug = serves::MinerDebugRPC::<P>::new(state);

    let mut io = IoHandler::default();

    io.extend_with(rpc.to_delegate());
    io.extend_with(filter.to_delegate());
    io.extend_with(debug.to_delegate());

    let server = ServerBuilder::new(io)
        .cors(DomainsValidation::AllowOnly(vec![
            AccessControlAllowOrigin::Any,
            AccessControlAllowOrigin::Null,
        ]))
        .start_http(addr)
        .expect("Expect to build HTTP RPC server");

    server.wait();
}
