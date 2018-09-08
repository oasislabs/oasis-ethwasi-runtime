#![feature(use_extern_macros)]

extern crate ethereum_types;
#[macro_use]
extern crate clap;
use clap::{App, Arg};
extern crate crossbeam_deque;
use crossbeam_deque::{Deque, Steal};
extern crate ekiden_instrumentation;
use ekiden_instrumentation::{measure, measure_gauge};
extern crate ekiden_instrumentation_prometheus;
extern crate filebuffer;
extern crate hex;
#[macro_use]
extern crate jsonrpc_client_core;
extern crate jsonrpc_client_http;
use jsonrpc_client_http::HttpTransport;
extern crate log;
use log::{info, LevelFilter};
extern crate pretty_env_logger;
extern crate prometheus;
use prometheus::Encoder;
extern crate rlp;
extern crate serde_derive;
extern crate serde_json;
extern crate threadpool;
use threadpool::ThreadPool;

use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

fn to_ms(d: Duration) -> f64 {
    d.as_secs() as f64 * 1e3 + d.subsec_nanos() as f64 * 1e-6
}

/// reads a file containing parity exported blocks and a max number of transactions to process
/// returns a queue of hex-encoded transactions
fn get_transactions_from_blocks(blocks_path: &str, max_num_transactions: usize) -> Deque<String> {
    let ret: Deque<String> = Deque::new();

    // Blocks are written one after another into the exported blocks file.
    // https://github.com/paritytech/parity/blob/v1.9.7/parity/blockchain.rs#L595
    let blocks_raw = filebuffer::FileBuffer::open(blocks_path).unwrap();
    let mut offset = 0;
    let mut num_transactions = 0;
    'outer: while offset < blocks_raw.len() {
        // Each block is a 3-list of (header, transactions, uncles).
        // https://github.com/paritytech/parity/blob/v1.9.7/ethcore/src/encoded.rs#L188
        let start = offset;
        let payload_info = rlp::PayloadInfo::from(&blocks_raw[start..]).unwrap();
        let end = start + payload_info.total();
        let block = rlp::Rlp::new(&blocks_raw[start..end]);
        offset = end;
        info!("Processing block at offset {}", start);
        // https://github.com/paritytech/parity/blob/v1.9.7/ethcore/src/views/block.rs#L101
        let transactions = block.at(1);
        for transaction in transactions.iter() {
            ret.push(format!("0x{}", hex::encode(transaction.as_raw())));
            num_transactions += 1;
            if max_num_transactions != 0 && num_transactions >= max_num_transactions {
                break 'outer;
            }
        }
    }

    ret
}

/// web3 JSON-RPC interface
jsonrpc_client!(pub struct Web3Client {
    pub fn eth_sendRawTransaction(&mut self, data: String) -> RpcRequest<String>;
});

fn main() {
    let args = App::new("Ethereum playback client")
        .arg(
            Arg::with_name("exported_blocks")
                .takes_value(true)
                .required(true),
        )
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
            Arg::with_name("transactions")
                .long("transactions")
                .takes_value(true)
                .default_value("0"),
        )
        .arg(
            Arg::with_name("threads")
                .long("threads")
                .takes_value(true)
                .default_value("1"),
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
        .get_matches();

    // Initialize logger.
    pretty_env_logger::formatted_builder()
        .unwrap()
        .filter(None, LevelFilter::Info)
        .init();

    // Initialize metrics.
    ekiden_instrumentation_prometheus::init().unwrap();

    // parity exported block file
    let blocks_path = value_t!(args, "exported_blocks", String).unwrap();
    // web3 provider host and port
    let host = value_t!(args, "host", String).unwrap();
    let port = value_t!(args, "port", String).unwrap();
    // maximum number of transactions to import
    let max_num_transactions = value_t!(args, "transactions", usize).unwrap();
    // number of requester threads
    let threads = value_t!(args, "threads", usize).unwrap();

    // pre-process all blocks into a queue of hex-encoded transactions
    let transactions = get_transactions_from_blocks(&blocks_path, max_num_transactions);
    let num_transactions = transactions.len();
    info!("Pre-processed {} transactions", num_transactions);

    let pool = ThreadPool::with_name("requesters".into(), threads);
    let playback_start = Instant::now();

    let (tx, rx) = channel();
    for _ in 0..pool.max_count() {
        let tx = tx.clone();
        let s = transactions.stealer();

        let transport = HttpTransport::new().unwrap();
        let transport_handle = transport
            .handle(&format!("http://{}:{}", host, port))
            .unwrap();

        pool.execute(move || {
            let mut client = Web3Client::new(transport_handle);
            let mut transaction_durs = vec![];
            loop {
                // get a transaction from the queue
                let transaction = match s.steal() {
                    Steal::Empty => break,
                    Steal::Data(data) => data,
                    Steal::Retry => continue,
                };
                let transaction_start = Instant::now();
                let res = client.eth_sendRawTransaction(transaction).call();
                let transaction_end = Instant::now();
                let transaction_dur = transaction_end - transaction_start;
                info!("eth_sendRawTransaction result: {:?}", res);
                transaction_durs.push(transaction_dur);
            }
            tx.send(transaction_durs).unwrap();
        });
    }
    pool.join();

    let playback_end = Instant::now();
    let playback_dur = playback_end - playback_start;
    let mut transaction_durs: Vec<Duration> = rx.try_iter().flat_map(|v| v).collect();
    let playback_dur_ms = to_ms(playback_dur);
    measure_gauge!("num_transactions", num_transactions);
    measure_gauge!("playback_dur_ms", playback_dur_ms);
    if num_transactions > 0 {
        let throughput_inv = playback_dur_ms / num_transactions as f64;
        measure_gauge!("throughput_inv", throughput_inv);
        let throughput = num_transactions as f64 / playback_dur_ms * 1000.;
        measure_gauge!("throughput", throughput);

        transaction_durs.sort();
        let latency_min = to_ms(*transaction_durs.first().unwrap());
        measure_gauge!("latency_min", latency_min);
        for pct in [1, 10, 50, 90, 99].iter() {
            let index = std::cmp::min(
                num_transactions - 1,
                (*pct as f64 / 100. * transaction_durs.len() as f64).ceil() as usize,
            );
            let latency_pct = to_ms(transaction_durs[index]);
            measure_gauge!(&format!("latency_{}", pct), latency_pct);
        }
        let latency_max = to_ms(*transaction_durs.last().unwrap());
        measure_gauge!("latency_max", latency_max);
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
