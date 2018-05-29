#![feature(use_extern_macros)]

extern crate protobuf;
extern crate serde;
extern crate sputnikvm;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate ekiden_core;

#[macro_use]
mod api;

// not using protobufs
//mod generated;
//pub use generated::api::*;

extern crate bigint;
pub use bigint::{Address, H256, U256};
mod state;
pub use state::*;
