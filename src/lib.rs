//! Example Ekiden contract.
#![feature(use_extern_macros)]

extern crate protobuf;

extern crate ekiden_core;
extern crate ekiden_trusted;

extern crate helloworld_api;

use helloworld_api::{with_api, HelloWorldRequest, HelloWorldResponse};

use ekiden_core::error::Result;
use ekiden_trusted::db::database_schema;
use ekiden_trusted::enclave::enclave_init;
use ekiden_trusted::key_manager::use_key_manager_contract;
use ekiden_trusted::rpc::create_enclave_rpc;

enclave_init!();

// Configure the key manager contract to use.
use_key_manager_contract!("generated/key-manager.identity");

// Create enclave RPC handlers.
with_api! {
    create_enclave_rpc!(api);
}

// Create database schema.
database_schema! {
    pub struct HelloWorldDb {
        pub counter: u64,
    }
}

pub fn hello_world(request: &HelloWorldRequest) -> Result<HelloWorldResponse> {
    let db = HelloWorldDb::new();
    let previous_counter = db.counter.get().unwrap_or(0);
    db.counter.insert(&(previous_counter + 1));

    let mut response = HelloWorldResponse::new();
    response.set_world(format!(
        "contract says {} ({} times)",
        request.hello, previous_counter,
    ));

    Ok(response)
}
