//! Common data structures shared by runtime and gateway.
extern crate elastic_array;
extern crate ethcore;
extern crate ethereum_types;
extern crate failure;
extern crate hashdb;
extern crate io_context;
extern crate keccak_hash;
extern crate lazy_static;
extern crate oasis_core_keymanager_client;
extern crate oasis_core_runtime;
extern crate vm;
extern crate zeroize;

pub mod confidential;
pub mod genesis;
pub mod parity;
pub mod storage;

/// Block gas limit.
pub const BLOCK_GAS_LIMIT: usize = 16_000_000;
/// Minimum gas price (in gwei).
pub const MIN_GAS_PRICE_GWEI: usize = 1;

/// Ethereum transaction hash tag (value is the Ethereum transaction hash).
pub const TAG_ETH_TX_HASH: &'static [u8] = b"heth";
/// Ethereum log address tag.
pub const TAG_ETH_LOG_ADDRESS: &'static [u8] = b"ladd";
/// Ethereum log topic tags.
pub const TAG_ETH_LOG_TOPICS: &'static [&[u8]; 4] = &[b"ltp1", b"ltp2", b"ltp3", b"ltp4"];
