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
    any::Any,
    sync::{Arc, Weak},
    thread,
    time::{Duration, Instant},
};

use client::Client;

use client_utils;
use ekiden_core::environment::Environment;
use ekiden_storage_base::StorageBackend;
use ethereum_types::U256;
use informant;
use parity_reactor::EventLoop;
use rpc::{self, HttpConfiguration, WsConfiguration};
use rpc_apis;

#[cfg(feature = "pubsub")]
use notifier::PubSubNotifier;
use runtime_ethereum;
use util;

pub fn execute(
    ekiden_client: runtime_ethereum::Client,
    snapshot_manager: client_utils::db::Manager,
    storage: Arc<StorageBackend>,
    environment: Arc<Environment>,
    pubsub_interval_secs: u64,
    http_port: u16,
    num_threads: usize,
    ws_port: u16,
    ws_max_connections: usize,
    ws_rate_limit: usize,
    gas_price: U256,
    jsonrpc_max_batch_size: usize,
) -> Result<RunningClient, String> {
    let client = Arc::new(Client::new(
        &util::load_spec(),
        snapshot_manager,
        ekiden_client,
        environment.clone(),
        storage.clone(),
        gas_price,
    ));

    #[cfg(feature = "pubsub")]
    let notifier = PubSubNotifier::new(client.clone(), environment.clone(), pubsub_interval_secs);

    let rpc_stats = Arc::new(informant::RpcStats::default());

    // spin up event loop
    let event_loop = EventLoop::spawn();

    // expose the http and ws servers to the world
    // conf corresponds to parity command-line options "--unsafe-expose" + "--jsonrpc-cors=all"
    let mut ws_conf = WsConfiguration::default();
    ws_conf.origins = None;
    ws_conf.hosts = None;
    ws_conf.interface = "0.0.0.0".into();
    ws_conf.port = ws_port;
    ws_conf.max_batch_size = jsonrpc_max_batch_size;
    ws_conf.max_req_per_sec = ws_rate_limit;

    // max # of concurrent connections. the default is 100, which is "low" and "should be increased":
    // https://github.com/tomusdrw/ws-rs/blob/f12d19c4c19422fc79af28a3181f598bc07ecd1e/src/lib.rs#L128
    ws_conf.max_connections = ws_max_connections;

    let mut http_conf = HttpConfiguration::default();
    http_conf.cors = None;
    http_conf.hosts = None;
    http_conf.interface = "0.0.0.0".into();
    http_conf.port = http_port;
    http_conf.processing_threads = num_threads;
    http_conf.max_batch_size = jsonrpc_max_batch_size;

    // start RPCs
    let deps_for_rpc_apis = Arc::new(rpc_apis::FullDependencies {
        client: client.clone(),
        ws_address: ws_conf.address(),
        remote: event_loop.remote(),
    });

    let dependencies = rpc::Dependencies {
        apis: deps_for_rpc_apis.clone(),
        remote: event_loop.raw_remote(),
        stats: rpc_stats.clone(),
    };

    // start rpc servers
    let ws_server = rpc::new_ws(ws_conf, &dependencies)?;
    let http_server = rpc::new_http("HTTP JSON-RPC", "jsonrpc", http_conf, &dependencies)?;

    #[cfg(feature = "pubsub")]
    let keep_alive_set = (event_loop, http_server, notifier, ws_server);
    #[cfg(not(feature = "pubsub"))]
    let keep_alive_set = (event_loop, http_server, ws_server);

    let running_client = RunningClient {
        inner: RunningClientInner::Full {
            client,
            keep_alive: Box::new(keep_alive_set),
        },
    };
    Ok(running_client)
}

/// Parity client currently executing in background threads.
///
/// Should be destroyed by calling `shutdown()`, otherwise execution will continue in the
/// background.
pub struct RunningClient {
    inner: RunningClientInner,
}

enum RunningClientInner {
    Full {
        client: Arc<Client>,
        keep_alive: Box<Any>,
    },
}

impl RunningClient {
    /// Shuts down the client.
    pub fn shutdown(self) {
        match self.inner {
            RunningClientInner::Full { client, keep_alive } => {
                info!("Finishing work, please wait...");
                // Create a weak reference to the client so that we can wait on shutdown
                // until it is dropped
                let weak_client = Arc::downgrade(&client);
                // drop this stuff as soon as exit detected.
                drop(keep_alive);
                drop(client);
                wait_for_drop(weak_client);
            }
        }
    }
}

fn wait_for_drop<T>(w: Weak<T>) {
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
            warn!("Shutdown is taking longer than expected.");
        }

        thread::sleep(sleep_duration);
    }

    warn!("Shutdown timeout reached, exiting uncleanly.");
}
