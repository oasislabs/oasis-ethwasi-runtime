#![feature(use_extern_macros)]

#[macro_use]
extern crate clap;
use clap::{App, Arg};
extern crate log;
use log::{debug, info, LevelFilter};
extern crate pretty_env_logger;

#[macro_use]
extern crate jsonrpc_client_core;
extern crate jsonrpc_client_http;
use jsonrpc_client_http::{HttpHandle, HttpTransport};

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
    debug!("result: {:?}", res);
}

fn eth_getBlockByNumber(client: &mut Web3Client<HttpHandle>) {
    let res = client
        .eth_getBlockByNumber("latest".to_string(), true)
        .call();
    debug!("result: {:?}", res);
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
    duration_millis: u64,
) {
    let time_before = Instant::now();
    let counter = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let mut thread_handles = vec![];
    thread_handles.reserve_exact(threads);
    info!("Starting {} benchmark...", name);
    for _ in 0..threads {
        let tl_counter = counter.clone();
        let tl_stop = stop.clone();
        let transport = HttpTransport::new().unwrap();
        let transport_handle = transport.handle(url).unwrap();
        thread_handles.push(std::thread::spawn(move || {
            let mut client = Web3Client::new(transport_handle);
            while !tl_stop.load(Ordering::Relaxed) {
                scenario(&mut client);
                tl_counter.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    let time_start = Instant::now();
    let count_start = counter.load(Ordering::Relaxed);
    info!("Threads started");

    // First 10% of time will be discarded.
    let ramp_up_millis = duration_millis / 10;
    std::thread::sleep(Duration::from_millis(ramp_up_millis));

    let time_mid_before = Instant::now();
    let count_mid_before = counter.load(Ordering::Relaxed);

    // Middle 80% of time will be counted.
    let mid_millis = duration_millis / 10 * 8;
    std::thread::sleep(Duration::from_millis(mid_millis));

    let time_mid_after = Instant::now();
    let count_mid_after = counter.load(Ordering::Relaxed);

    // Last 10% of time will be discarded.
    let ramp_down_millis = duration_millis / 10;
    std::thread::sleep(Duration::from_millis(ramp_down_millis));

    let time_end = Instant::now();
    let count_end = counter.load(Ordering::Relaxed);
    info!("Done, joining threads");

    stop.store(true, Ordering::Relaxed);
    for thread_handle in thread_handles.into_iter() {
        thread_handle.join().unwrap();
    }

    let time_after = Instant::now();
    let count_after = counter.load(Ordering::Relaxed);
    info!("Threads joined");

    let mid_count = count_mid_after - count_mid_before;
    let mid_dur = time_mid_after - time_mid_before;
    let mid_dur_ms = to_ms(mid_dur);
    let throughput_inv = mid_dur_ms / mid_count as f64;
    let throughput = mid_count as f64 / mid_dur_ms * 1000.;
    println!("# TYPE {}_mid_count gauge", name);
    println!("# HELP {}_mid_count {} call count", name, name);
    println!("{}_mid_count {}", name, mid_count);
    println!("# TYPE {}_mid_dur_ms gauge", name);
    println!("# HELP {}_mid_dur_ms Total time (ms)", name);
    println!("{}_dur_ms {}", name, mid_dur_ms);
    println!("# TYPE {}_throughput_inv gauge", name);
    println!("# HELP {}_throughput_inv Inverse throughput (ms/tx)", name);
    println!("{}_throughput_inv {}", name, throughput_inv);
    println!("# TYPE {}_throughput gauge", name);
    println!("# HELP {}_throughput Throughput (tx/sec)", name);
    println!("{}_throughput {}", name, throughput);

    let total_count = count_end - count_start;
    let total_dur = time_end - time_start;
    let total_dur_ms = to_ms(total_dur);
    info!(
        "Overall {}: {:?} calls over {:.3} ms ({:.3} calls/sec)",
        name,
        total_count,
        total_dur_ms,
        total_count as f64 / total_dur_ms * 1000.
    );

    let before_count = count_start;
    let before_dur = time_start - time_before;
    let before_dur_ms = to_ms(before_dur);
    info!(
        "Ramp up {}: {:?} calls over {:.3} ms ({:.3} calls/sec)",
        name,
        before_count,
        before_dur_ms,
        before_count as f64 / before_dur_ms * 1000.
    );

    let after_count = count_after - count_end;
    let after_dur = time_after - time_end;
    let after_dur_ms = to_ms(after_dur);
    info!(
        "Ramp down {}: {:?} calls over {:.3} ms ({:.3} calls/sec)",
        name,
        after_count,
        after_dur_ms,
        after_count as f64 / after_dur_ms * 1000.
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
        .arg(
            Arg::with_name("benchmark")
                .required(true)
                .multiple(true)
                .possible_values(&[
                    "eth_blockNumber",
                    "net_version",
                    "eth_getBlockByNumber",
                    "debug_nullCall",
                    "transfer",
                ]),
        )
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

    for benchmark in args.values_of("benchmark").unwrap() {
        match benchmark {
            "eth_blockNumber" => {
                run_scenario("eth_blockNumber", eth_blockNumber, &url, threads, 30000)
            }
            "net_version" => run_scenario("net_version", net_version, &url, threads, 30000),
            "eth_getBlockByNumber" => run_scenario(
                "eth_getBlockByNumber",
                eth_getBlockByNumber,
                &url,
                threads,
                30000,
            ),
            "debug_nullCall" => run_scenario("null call", debug_nullCall, &url, threads, 30000),
            "transfer" => unimplemented!(),
            _ => unreachable!(),
        }
    }
}
