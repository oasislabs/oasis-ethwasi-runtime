#![feature(use_extern_macros)]

#[macro_use]
extern crate clap;
extern crate futures;
extern crate rand;
extern crate grpcio;

#[macro_use]
extern crate client_utils;
extern crate ekiden_core;
extern crate ekiden_rpc_client;

extern crate helloworld_api;

use clap::{App, Arg};
use futures::future::Future;

use ekiden_rpc_client::create_client_rpc;
use helloworld_api::with_api;

with_api! {
    create_client_rpc!(helloworld, helloworld_api, api);
}

fn main() {
    let mut client = contract_client!(helloworld);

    // Send some text.
    let mut request = helloworld::HelloWorldRequest::new();
    request.set_hello("hello from client".to_string());

    // Call contract method and check the response.
    let response = client.hello_world(request).wait().unwrap();

    println!("response: {}", response.get_world());
}
