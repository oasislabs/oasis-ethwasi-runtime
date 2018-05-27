#![feature(use_extern_macros)]

extern crate protobuf;
extern crate serde;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate ekiden_core;

#[macro_use]
mod api;
mod generated;

pub use generated::api::*;

extern crate bigint;
mod state;
pub use state::*;
