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

use std::{
    sync::{Arc, Weak},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Error, Result};
use ethereum_types::U256;
use informant;
use oasis_core_keymanager_client::KeyManagerClient;
use oasis_core_runtime::common::logger::get_logger;
use rpc::{self, HttpConfiguration, WsConfiguration};
use rpc_apis;
use slog::{info, warn, Logger};

use crate::{pubsub::Broker, translator::Translator, EthereumRuntimeClient};

pub fn execute(
    client: EthereumRuntimeClient,
    km_client: Arc<dyn KeyManagerClient>,
    pubsub_interval_secs: u64,
    interface: &str,
    http_port: u16,
    num_threads: usize,
    ws_port: u16,
    ws_max_connections: usize,
    ws_rate_limit: usize,
    gas_price: U256,
    jsonrpc_max_batch_size: usize,
) -> Result<RunningGateway> {
    let logger = get_logger("gateway/execute");

    let mut runtime = tokio::runtime::Runtime::new()?;

    // Wait for the Oasis Core node to be fully synced.
    info!(logger, "Waiting for the Oasis Core node to be fully synced");
    runtime.block_on(client.txn_client().wait_sync())?;
    info!(
        logger,
        "Oasis Core node is fully synced, proceeding with initialization"
    );

    let translator = Arc::new(Translator::new(client, gas_price));
    let broker = Arc::new(Broker::new(translator.clone()));
    runtime.spawn(broker.start(Duration::new(pubsub_interval_secs, 0)));

    let rpc_stats = Arc::new(informant::RpcStats::default());

    // expose the http and ws servers to the world
    // conf corresponds to parity command-line options "--unsafe-expose" + "--jsonrpc-cors=all"
    let mut ws_conf = WsConfiguration::default();
    ws_conf.origins = None;
    ws_conf.hosts = None;
    ws_conf.interface = interface.into();
    ws_conf.port = ws_port;
    ws_conf.max_batch_size = jsonrpc_max_batch_size;
    ws_conf.max_req_per_sec = ws_rate_limit;

    // max # of concurrent connections. the default is 100, which is "low" and "should be increased":
    // https://github.com/tomusdrw/ws-rs/blob/f12d19c4c19422fc79af28a3181f598bc07ecd1e/src/lib.rs#L128
    ws_conf.max_connections = ws_max_connections;

    let mut http_conf = HttpConfiguration::default();
    http_conf.cors = None;
    http_conf.hosts = None;
    http_conf.interface = interface.into();
    http_conf.port = http_port;
    http_conf.server_threads = num_threads;
    http_conf.max_batch_size = jsonrpc_max_batch_size;

    // Define RPC handlers.
    let deps_for_rpc_apis = Arc::new(rpc_apis::FullDependencies {
        translator: translator.clone(),
        broker: broker.clone(),
        km_client: km_client.clone(),
        ws_address: ws_conf.address(),
    });

    let dependencies = rpc::Dependencies {
        apis: deps_for_rpc_apis.clone(),
        executor: runtime.executor(),
        stats: rpc_stats.clone(),
    };

    // Start RPC servers.
    info!(logger, "Starting WS server"; "conf" => ?ws_conf);
    let ws_server = rpc::new_ws(ws_conf, &dependencies).map_err(Error::msg)?;

    info!(logger, "Starting HTTP server"; "conf" => ?http_conf);
    let http_server =
        rpc::new_http("HTTP JSON-RPC", "jsonrpc", http_conf, &dependencies).map_err(Error::msg)?;

    let running_client = RunningGateway {
        logger,
        runtime,
        translator,
        km_client,
        http_server,
        ws_server,
    };
    Ok(running_client)
}

/// Gateway currently executing in background threads.
///
/// Should be destroyed by calling `shutdown()`, otherwise execution will continue in the
/// background.
pub struct RunningGateway {
    logger: Logger,
    runtime: tokio::runtime::Runtime,
    translator: Arc<Translator>,
    km_client: Arc<dyn KeyManagerClient>,
    http_server: Option<jsonrpc_http_server::Server>,
    ws_server: Option<jsonrpc_ws_server::Server>,
}

impl RunningGateway {
    /// Shuts down the gateway.
    pub fn shutdown(self) {
        let RunningGateway {
            logger,
            runtime,
            translator,
            km_client,
            http_server,
            ws_server,
        } = self;

        info!(logger, "Terminating event loop");

        // Create a weak reference to the client so that we can wait on shutdown
        // until it is dropped.
        let weak_translator = Arc::downgrade(&translator);
        // drop this stuff as soon as exit detected.
        drop(runtime.shutdown_now());
        drop(http_server);
        drop(ws_server);
        drop(translator);
        drop(km_client);

        wait_for_drop(logger, weak_translator);
    }
}

fn wait_for_drop<T>(logger: Logger, w: Weak<T>) {
    let sleep_duration = Duration::from_secs(1);
    let warn_timeout = Duration::from_secs(60);
    let max_timeout = Duration::from_secs(300);

    let instant = Instant::now();
    let mut warned = false;

    while instant.elapsed() < max_timeout {
        if w.upgrade().is_none() {
            return;
        }

        if !warned && instant.elapsed() > warn_timeout {
            warned = true;
            warn!(logger, "Shutdown is taking longer than expected");
        }

        thread::sleep(sleep_duration);
    }

    warn!(logger, "Shutdown timeout reached, exiting uncleanly");
}
