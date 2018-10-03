#[macro_use]
extern crate clap;
use clap::{App, Arg};
extern crate ekiden_instrumentation;
use ekiden_instrumentation::{measure, measure_gauge};
extern crate ekiden_instrumentation_prometheus;
#[macro_use]
extern crate lazy_static;
extern crate log;
use log::{debug, error, info, LevelFilter};
extern crate pretty_env_logger;
extern crate prometheus;
use prometheus::Encoder;
extern crate rand;
use rand::Rng;

#[macro_use]
extern crate jsonrpc_client_core;
extern crate jsonrpc_client_http;
use jsonrpc_client_http::{HttpHandle, HttpTransport};

extern crate ethcore_transaction;
extern crate ethkey;
use ethkey::Generator;
extern crate ethereum_types;
use ethereum_types::{Address, U256};

use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

extern crate hex;
extern crate rlp;
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
    pub fn eth_sendRawTransaction(&mut self, data: String) -> RpcRequest<String>;
});

fn no_prep(_client: &mut Web3Client<HttpHandle>) -> () {
    ()
}

// scenarios
fn eth_blockNumber(client: &mut Web3Client<HttpHandle>, _context: &mut ()) {
    let res = client.eth_blockNumber().call();
    debug!("result: {:?}", res);
}

fn eth_getBlockByNumber(client: &mut Web3Client<HttpHandle>, _context: &mut ()) {
    let res = client
        .eth_getBlockByNumber("latest".to_string(), true)
        .call();
    debug!("result: {:?}", res);
}

fn net_version(client: &mut Web3Client<HttpHandle>, _context: &mut ()) {
    let res = client.net_version().call();
    debug!("result: {:?}", res);
}

fn debug_nullCall(client: &mut Web3Client<HttpHandle>, _context: &mut ()) {
    let res = client.debug_nullCall().call();
    debug!("result: {:?}", res);
}

struct TransferAccount {
    keypair: ethkey::KeyPair,
    nonce: u64,
}

impl TransferAccount {
    fn new() -> Self {
        TransferAccount {
            keypair: ethkey::Random.generate().unwrap(),
            nonce: 0,
        }
    }
}

lazy_static! {
    static ref FUND_ACCOUNT: Mutex<TransferAccount> = Mutex::new(TransferAccount {
        // address: 0x7110316b618d20d0c44728ac2a3d683536ea682
        keypair: ethkey::KeyPair::from_secret(
            ethkey::Secret::from_str(
                "533d62aea9bbcb821dfdda14966bb01bfbbb53b7e9f5f0d69b8326e052e3450c",
            ).unwrap(),
        ).unwrap(),
        nonce: 0,
    });
}

fn transfer_prep(client: &mut Web3Client<HttpHandle>) -> TransferAccount {
    let account = TransferAccount::new();
    let mut fund_account = FUND_ACCOUNT.lock().unwrap();
    let recipient = account.keypair.address();
    let tx = ethcore_transaction::Transaction {
        nonce: U256::from(fund_account.nonce),
        gas_price: U256::from(1_000_000_000),
        gas: U256::from(1000000),
        action: ethcore_transaction::Action::Call(recipient),
        value: U256::from(100_000_000_000_000_000u64),
        data: vec![],
    }.sign(fund_account.keypair.secret(), None);
    let tx_raw = rlp::encode(&tx);
    let tx_hex = format!("0x{}", hex::encode(tx_raw));
    client.eth_sendRawTransaction(tx_hex).call().unwrap();
    fund_account.nonce += 1;
    info!("Funded account, fund_account nonce {}", fund_account.nonce);
    account
}

fn transfer(client: &mut Web3Client<HttpHandle>, account: &mut TransferAccount) {
    let mut recipient = Address::zero();
    rand::thread_rng().fill_bytes(&mut recipient.0);
    let tx = ethcore_transaction::Transaction {
        nonce: U256::from(account.nonce),
        gas_price: U256::from(1_000_000_000),
        gas: U256::from(1000000),
        action: ethcore_transaction::Action::Call(recipient),
        value: U256::one(),
        data: vec![],
    }.sign(account.keypair.secret(), None);
    let tx_raw = rlp::encode(&tx);
    let tx_hex = format!("0x{}", hex::encode(tx_raw));
    let res = client.eth_sendRawTransaction(tx_hex).call();
    if let Err(e) = res {
        error!("Transaction failed: {:?}", e);
    } else {
        account.nonce += 1;
    }
}

fn run_scenario<C: Send + 'static>(
    name: &str,
    prep: fn(&mut Web3Client<HttpHandle>) -> C,
    scenario: fn(&mut Web3Client<HttpHandle>, &mut C),
    url: &str,
    threads: usize,
    duration_millis: u64,
) {
    info!("Starting {} benchmark...", name);
    let mut contexts = vec![];
    contexts.reserve_exact(threads);
    for _ in 0..threads {
        let transport = HttpTransport::new().unwrap();
        let transport_handle = transport.handle(url).unwrap();
        let mut client = Web3Client::new(transport_handle);
        let context = prep(&mut client);
        contexts.push((client, context));
    }
    info!("Preparation done");

    let counter = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let mut thread_handles = vec![];
    thread_handles.reserve_exact(threads);
    let time_before = Instant::now();
    for (mut client, mut context) in contexts.into_iter() {
        let tl_counter = counter.clone();
        let tl_stop = stop.clone();
        thread_handles.push(std::thread::spawn(move || {
            while !tl_stop.load(Ordering::Relaxed) {
                scenario(&mut client, &mut context);
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
    info!("Begin middle 80%");

    // Middle 80% of time will be counted.
    let mid_millis = duration_millis / 10 * 8;
    std::thread::sleep(Duration::from_millis(mid_millis));

    let time_mid_after = Instant::now();
    let count_mid_after = counter.load(Ordering::Relaxed);
    info!("End middle 80%");

    // Last 10% of time will be discarded.
    // This might not be as important, the way this benchmark works.
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
    info!(
        "Middle 80% {}: {} calls over {:.3} ms ({:.3} calls/sec)",
        name, mid_count, mid_dur_ms, throughput,
    );
    measure_gauge!(&format!("{}_mid_count", name), mid_count);
    measure_gauge!(&format!("{}_mid_dur_ms", name), mid_dur_ms);
    measure_gauge!(&format!("{}_throughput_inv", name), throughput_inv);
    measure_gauge!(&format!("{}_throughput", name), throughput);

    let total_count = count_end - count_start;
    let total_dur = time_end - time_start;
    let total_dur_ms = to_ms(total_dur);
    info!(
        "Overall {}: {} calls over {:.3} ms ({:.3} calls/sec)",
        name,
        total_count,
        total_dur_ms,
        total_count as f64 / total_dur_ms * 1000.
    );

    let before_count = count_start;
    let before_dur = time_start - time_before;
    let before_dur_ms = to_ms(before_dur);
    info!(
        "Ramp up {}: {} calls over {:.3} ms ({:.3} calls/sec)",
        name,
        before_count,
        before_dur_ms,
        before_count as f64 / before_dur_ms * 1000.
    );

    let after_count = count_after - count_end;
    let after_dur = time_after - time_end;
    let after_dur_ms = to_ms(after_dur);
    info!(
        "Ramp down {}: {} calls over {:.3} ms ({:.3} calls/sec)",
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
        .arg(
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
        .arg(
            Arg::with_name("prometheus-push-addr")
                .long("prometheus-push-addr")
                .help("Send results to the Prometheus push gateway at the given address.")
                .requires("prometheus-push-job-name")
                .requires("prometheus-push-instance-label")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("prometheus-push-job-name")
                .long("prometheus-push-job-name")
                .help("Prometheus `job` name used if sending results.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("prometheus-push-instance-label")
                .long("prometheus-push-instance-label")
                .help("Prometheus `instance` label used if using push mode.")
                .takes_value(true),
        )
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
        .filter(
            None,
            match args.occurrences_of("v") {
                0 => LevelFilter::Info,
                1 => LevelFilter::Debug,
                _ => LevelFilter::max(),
            },
        )
        .init();

    // Initialize metrics.
    ekiden_instrumentation_prometheus::init().unwrap();

    let host = value_t!(args, "host", String).unwrap();
    let port = value_t!(args, "port", String).unwrap();
    let threads = value_t!(args, "threads", usize).unwrap();
    let url = format!("http://{}:{}", host, port);

    for benchmark in args.values_of("benchmark").unwrap() {
        match benchmark {
            "eth_blockNumber" => run_scenario(
                "eth_blockNumber",
                no_prep,
                eth_blockNumber,
                &url,
                threads,
                30000,
            ),
            "net_version" => {
                run_scenario("net_version", no_prep, net_version, &url, threads, 30000)
            }
            "eth_getBlockByNumber" => run_scenario(
                "eth_getBlockByNumber",
                no_prep,
                eth_getBlockByNumber,
                &url,
                threads,
                30000,
            ),
            "debug_nullCall" => {
                run_scenario("null_call", no_prep, debug_nullCall, &url, threads, 30000)
            }
            "transfer" => run_scenario("transfer", transfer_prep, transfer, &url, threads, 30000),
            _ => unreachable!(),
        }
    }

    let encoder = prometheus::TextEncoder::new();
    encoder
        .encode(&prometheus::gather(), &mut std::io::stdout())
        .unwrap();
    if let Some(addr) = args.value_of("prometheus-push-addr") {
        let job_name = args.value_of("prometheus-push-job-name").unwrap();
        let instance_label = args.value_of("prometheus-push-instance-label").unwrap();
        ekiden_instrumentation_prometheus::push::push_metrics(addr, job_name, instance_label)
            .unwrap();
    }
}
