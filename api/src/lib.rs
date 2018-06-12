#![feature(use_extern_macros)]

extern crate common_types as ethcore_types;
extern crate protobuf;
extern crate serde;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate ekiden_core;

#[macro_use]
mod api;

extern crate ethereum_types;
pub use ethereum_types::{Address, H256, U256};

mod generated;
pub use generated::api::*;
mod state;
pub use state::*;

pub mod error;
