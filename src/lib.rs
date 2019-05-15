//! Ethereum runtime.
#[cfg(feature = "test")]
extern crate byteorder;
extern crate bytes;
extern crate ekiden_client;
extern crate ekiden_keymanager_client;
extern crate ekiden_runtime;
#[cfg(feature = "test")]
extern crate elastic_array;
extern crate ethcore;
extern crate ethereum_types;
#[cfg(feature = "test")]
extern crate ethkey;
extern crate failure;
extern crate io_context;
extern crate lazy_static;
extern crate runtime_ethereum_api;
extern crate runtime_ethereum_common;
#[cfg(feature = "test")]
#[macro_use]
extern crate serde_json;
extern crate slog;

pub mod block;
pub mod methods;

#[cfg(feature = "test")]
pub mod test;

// Include key manager enclave hash.
include!(concat!(env!("OUT_DIR"), "/km_enclave_hash.rs"));
