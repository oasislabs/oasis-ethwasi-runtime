#![feature(use_extern_macros)]

#[macro_use]
extern crate clap;
use clap::{App, Arg};
extern crate log;
use log::{debug, info, log, LevelFilter};
extern crate pretty_env_logger;

extern crate threadpool;
use threadpool::ThreadPool;

#[macro_use]
extern crate jsonrpc_client_core;
extern crate jsonrpc_client_http;
use jsonrpc_client_http::{HttpHandle, HttpTransport};

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

// scenarios
fn eth_blockNumber(client: &mut Web3Client<HttpHandle>) {
    let res = client.eth_blockNumber().call();
    info!("result: {:?}", res);
}

fn eth_getBlockByNumber(client: &mut Web3Client<HttpHandle>) {
    let res = client
        .eth_getBlockByNumber("latest".to_string(), true)
        .call();
    info!("result: {:?}", res);
}

fn net_version(client: &mut Web3Client<HttpHandle>) {
    let res = client.net_version().call();
    debug!("result: {:?}", res);
}

fn debug_nullCall(client: &mut Web3Client<HttpHandle>) {
    let res = client.debug_nullCall().call();
    debug!("result: {:?}", res);
}

fn run_scenario(
    name: &str,
    scenario: fn(&mut Web3Client<HttpHandle>),
    url: &str,
    threads: usize,
    number: usize,
) {
    debug!("Starting {} benchmark...", name);
    let pool = ThreadPool::with_name("clients".into(), threads);
    let counter = Arc::new(AtomicUsize::new(0));
    let start = Instant::now();

    for _ in 0..pool.max_count() {
        let counter = counter.clone();
        let transport = HttpTransport::new().unwrap();
        let transport_handle = transport.handle(&url).unwrap();

        pool.execute(move || {
            let mut client = Web3Client::new(transport_handle);
            loop {
                scenario(&mut client);
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
    debug!(
        "{}: {:?} calls over {:.3} ms ({:.3} calls/sec)",
        name,
        total,
        to_ms(duration),
        total as f64 / to_ms(duration) * 1000.
    );
}

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
            Arg::with_name("threads")
                .long("threads")
                .takes_value(true)
                .default_value("1"),
        )
        .arg(Arg::with_name("v")
             .short("v")
             .multiple(true)
             .help("Sets the level of verbosity"))
        .get_matches();

    // Initialize logger.
    pretty_env_logger::formatted_builder()
        .unwrap()
        .filter( None, match args.occurrences_of("v") {
            0 => LevelFilter::Info,
            1 => LevelFilter::Debug,
            _ => LevelFilter::max(),
        }).init();

    let host = value_t!(args, "host", String).unwrap();
    let port = value_t!(args, "port", String).unwrap();
    let threads = value_t!(args, "threads", usize).unwrap();
    let url = format!("http://{}:{}", host, port);

    run_scenario("eth_blockNumber", eth_blockNumber, &url, threads, 5000);
    run_scenario("net_version", net_version, &url, threads, 100000);
    run_scenario(
        "eth_getBlockByNumber",
        eth_getBlockByNumber,
        &url,
        threads,
        5000,
    );
    run_scenario("null call", debug_nullCall, &url, threads, 5000);
}
