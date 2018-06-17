#![feature(use_extern_macros)]

#[macro_use]
extern crate clap;
use clap::{App, Arg};
extern crate log;
use log::{info, log, LevelFilter};
extern crate pretty_env_logger;

extern crate threadpool;
use threadpool::ThreadPool;

#[macro_use]
extern crate jsonrpc_client_core;
extern crate jsonrpc_client_http;
use jsonrpc_client_http::HttpTransport;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

extern crate serde_json;
use serde_json::Value;

fn to_ms(d: Duration) -> f64 {
    d.as_secs() as f64 * 1e3 + d.subsec_nanos() as f64 * 1e-6
}

// web3 JSON-RPC interface
jsonrpc_client!(pub struct Web3Client {
    pub fn eth_blockNumber(&mut self) -> RpcRequest<String>;
    pub fn eth_getBlockByNumber(&mut self, number: String, full: bool) -> RpcRequest<Value>;
    pub fn debug_nullCall(&mut self) -> RpcRequest<bool>;
    pub fn net_version(&mut self) -> RpcRequest<String>;
});

fn main() {
    let args = App::new("web3 benchmarking client")
        .arg(
            Arg::with_name("host")
                .long("host")
                .takes_value(true)
                .default_value("127.0.0.1"),
        )
        .arg(
            Arg::with_name("port")
                .long("port")
                .takes_value(true)
                .default_value("8545"),
        )
        .arg(
            Arg::with_name("number")
                .long("number")
                .takes_value(true)
                .default_value("1"),
        )
        .arg(
            Arg::with_name("threads")
                .long("threads")
                .takes_value(true)
                .default_value("1"),
        )
        .get_matches();

    // Initialize logger.
    pretty_env_logger::formatted_builder()
        .unwrap()
        .filter(None, LevelFilter::Info)
        .init();

    let host = value_t!(args, "host", String).unwrap();
    let port = value_t!(args, "port", String).unwrap();
    let number = value_t!(args, "number", usize).unwrap();
    let threads = value_t!(args, "threads", usize).unwrap();

    let pool = ThreadPool::with_name("clients".into(), threads);
    let counter = Arc::new(AtomicUsize::new(0));

    let url = format!("http://{}:{}", host, port);

    let start = Instant::now();

    for _ in 0..pool.max_count() {
        let counter = counter.clone();
        let transport = HttpTransport::new().unwrap();
        let transport_handle = transport.handle(&url).unwrap();

        pool.execute(move || {
            let mut client = Web3Client::new(transport_handle);
            loop {
                //let res = client
                //    .eth_getBlockByNumber("latest".to_string(), false)
                //    .call();
                //let res = client.net_version().call();
                let res = client.debug_nullCall().call();
                //let res = client.eth_blockNumber().call();
                //info!("Result: {:?}", res);
                if counter.fetch_add(1, Ordering::Relaxed) >= number {
                    break;
                }
            }
        });
    }
    pool.join();

    let end = Instant::now();
    let total = counter.load(Ordering::SeqCst);
    let duration = end - start;
    info!(
        "Executed {:?} web3 calls over {:.3} ms",
        total,
        to_ms(duration)
    );
    info!(
        "Throughput: {:.3} calls/sec",
        total as f64 / to_ms(duration) * 1000.
    );
}
