// Copyright 2015-2018 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! web3 gateway for Oasis Eth/WASI runtime.

#![deny(warnings)]

extern crate fdlimit;
extern crate signal_hook;
#[macro_use]
extern crate clap;
extern crate failure;
extern crate log;
extern crate oasis_core_runtime;
extern crate oasis_ethwasi_runtime_common;
extern crate prometheus;
extern crate slog;
extern crate web3_gateway;

mod metrics;

use std::{io::Read, net::SocketAddr, os::unix::net::UnixStream, time::Duration};

use clap::{App, Arg};
use failure::Fallible;
use fdlimit::raise_fd_limit;
use slog::{error, info};

use oasis_core_runtime::common::logger::{get_logger, init_logger};
use oasis_ethwasi_runtime_common::MIN_GAS_PRICE_GWEI;
use web3_gateway::util;

const METRICS_MODE_PULL: &str = "pull";
const METRICS_MODE_PUSH: &str = "push";

fn main() -> Fallible<()> {
    // TODO: is this needed?
    // increase max number of open files
    raise_fd_limit();

    let gas_price = MIN_GAS_PRICE_GWEI.to_string();

    let args = App::new("Oasis Eth/WASI Runtime Web3 Gateway")
        .arg(
            Arg::with_name("runtime-id")
                .long("runtime-id")
                .help("Oasis Core runtime identifier for the runtime")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("node-address")
                .long("node-address")
                .help("Oasis Core node address")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("http-port")
                .long("http-port")
                .help("Port to use for JSON-RPC HTTP server.")
                .default_value("8545")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("threads")
                .long("threads")
                .help("Number of threads to use for HTTP server.")
                .default_value("1")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("ws-port")
                .long("ws-port")
                .help("Port to use for WebSocket server.")
                .default_value("8546")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("ws-max-connections")
                .long("ws-max-connections")
                .help("Max number of concurrent WebSocket connections.")
                .default_value("10000")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("ws-rate-limit")
                .long("ws-rate-limit")
                .help("Max requests/second allowed on a WebSocket connection.")
                .default_value("50")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("pubsub-interval")
                .long("pubsub-interval")
                .help("Time interval used for pub/sub notifications (in sec).")
                .default_value("3")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("gas-price")
                .long("gas-price")
                .help("Gas price (in Gwei).")
                .default_value(&gas_price)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("jsonrpc-max-batch")
                .long("jsonrpc-max-batch")
                .help("Max number of JSON-RPC calls allowed in a batch.")
                .default_value("10")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("interface")
                .long("interface")
                .help("Interface address for HTTP and WebSocket servers.")
                .default_value("127.0.0.1")
                .takes_value(true),
        )
        // Metrics.
        .arg(
            Arg::with_name("prometheus-mode")
            .long("prometheus-mode")
            .possible_values(&[METRICS_MODE_PULL, METRICS_MODE_PUSH])
            .takes_value(true),
        )
        .arg(
            Arg::with_name("prometheus-push-interval")
            .long("prometheus-push-interval")
            .help("Push interval in seconds (only used if using push mode).")
            .takes_value(true),
        )
        .arg(
            Arg::with_name("prometheus-push-job-name")
            .long("prometheus-push-job-name")
            .help("Prometheus `job` name used if using push mode.")
            .required_if("prometheus-mode", METRICS_MODE_PUSH)
            .takes_value(true),
        )
        .arg(
            Arg::with_name("prometheus-push-instance-label")
            .long("prometheus-push-instance-label")
            .help("Prometheus `instance` label used if using push mode.")
            .required_if("prometheus-mode", METRICS_MODE_PUSH)
            .takes_value(true),
        )
        .arg(
            Arg::with_name("prometheus-metrics-addr")
            .long("prometheus-metrics-addr")
            .requires("prometheus-mode")
            .help("If pull mode: A SocketAddr (as a string) from which to serve metrics to Prometheus. If push mode: prometheus 'pushgateway' address.")
            .takes_value(true)
        )
        // Logging.
        .arg(
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
        .get_matches();

    let log_level = match args.occurrences_of("v") {
        0 => log::LogLevel::Error,
        1 => log::LogLevel::Warn,
        2 => log::LogLevel::Debug,
        3 => log::LogLevel::Info,
        4 | _ => log::LogLevel::Trace,
    };

    // Initializes the log -> slog adapter so that we can use the log crate, e.g., in Parity.
    init_logger(log_level);
    let logger = get_logger("gateway/main");

    let num_threads = value_t!(args, "threads", usize)?;
    let interface = value_t!(args, "interface", String)?;
    let http_port = value_t!(args, "http-port", u16)?;
    let ws_port = value_t!(args, "ws-port", u16)?;
    let ws_max_connections = value_t!(args, "ws-max-connections", usize)?;
    let ws_rate_limit = value_t!(args, "ws-rate-limit", usize)?;
    let pubsub_interval_secs = value_t!(args, "pubsub-interval", u64)?;
    let gas_price = util::gwei_to_wei(value_t!(args, "gas-price", u64)?);
    let jsonrpc_max_batch_size = value_t!(args, "jsonrpc-max-batch", usize)?;

    // Metrics.
    match args.value_of("prometheus-mode") {
        Some(METRICS_MODE_PULL) => {
            if let Ok(address) = value_t!(args, "prometheus-metrics-addr", SocketAddr) {
                metrics::start(metrics::Config::Pull { address });
            }
        }
        Some(METRICS_MODE_PUSH) => {
            if let Ok(address) = value_t!(args, "prometheus-metrics-addr", String) {
                let interval = value_t!(args, "prometheus-push-interval", u64).unwrap_or(5);
                let job_name = value_t!(args, "prometheus-push-job-name", String).unwrap();
                let instance_name =
                    value_t!(args, "prometheus-push-instance-label", String).unwrap();

                metrics::start(metrics::Config::Push {
                    address,
                    period: Duration::from_secs(interval),
                    job_name,
                    instance_name,
                });
            }
        }
        _ => (),
    }

    info!(logger, "Starting the web3 gateway");

    let client = web3_gateway::start(
        args,
        pubsub_interval_secs,
        &interface,
        http_port,
        num_threads,
        ws_port,
        ws_max_connections,
        ws_rate_limit,
        gas_price,
        jsonrpc_max_batch_size,
    );

    let client = match client {
        Ok(client) => client,
        Err(err) => {
            error!(logger, "Failed to initialize web3 gateway"; "err" => ?err);
            return Ok(());
        }
    };

    info!(logger, "Web3 gateway is running");

    // Register a self-pipe for handing the SIGTERM and SIGINT signals.
    let (mut read, write) = UnixStream::pair()?;
    signal_hook::pipe::register(signal_hook::SIGINT, write.try_clone()?)?;
    signal_hook::pipe::register(signal_hook::SIGTERM, write.try_clone()?)?;

    // Wait for signal.
    let mut buff = [0];
    read.read_exact(&mut buff)?;

    info!(logger, "The web3 gateway is shutting down");

    client.shutdown();

    info!(logger, "Shutdown completed");

    Ok(())
}
