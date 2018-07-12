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

use std::any::Any;
use std::sync::{Arc, Weak};
use std::thread;
use std::time::{Duration, Instant};

use client::Client;

use futures_cpupool::CpuPool;
use jsonrpc_core;
use parity_reactor::EventLoop;
use parity_rpc::{informant, Metadata, Origin};
use rpc::{self, HttpConfiguration, WsConfiguration};
use rpc_apis;

use runtime_ethereum;

pub fn execute(
    ekiden_client: runtime_ethereum::Client,
    num_threads: usize,
) -> Result<RunningClient, String> {
    let client = Arc::new(Client::new(ekiden_client));
    let rpc_stats = Arc::new(informant::RpcStats::default());

    // spin up event loop
    let event_loop = EventLoop::spawn();

    let cpu_pool = CpuPool::new(4);

    let ws_conf = WsConfiguration::default();
    let mut http_conf = HttpConfiguration::default();
    http_conf.processing_threads = num_threads;

    // start RPCs
    let deps_for_rpc_apis = Arc::new(rpc_apis::FullDependencies {
        client: client.clone(),
        ws_address: ws_conf.address(),
        pool: cpu_pool.clone(),
        remote: event_loop.remote(),
    });

    let dependencies = rpc::Dependencies {
        apis: deps_for_rpc_apis.clone(),
        remote: event_loop.raw_remote(),
        stats: rpc_stats.clone(),
        pool: Some(rpc::CpuPool::new(http_conf.processing_threads)),
    };

    // start rpc servers
    let rpc_direct = rpc::setup_apis(rpc_apis::ApiSet::All, &dependencies);
    // WebSocket endpoint is disabled
    //let ws_server = rpc::new_ws(ws_conf, &dependencies)?;
    let http_server = rpc::new_http("HTTP JSON-RPC", "jsonrpc", http_conf, &dependencies)?;

    Ok(RunningClient {
        inner: RunningClientInner::Full {
            rpc: rpc_direct,
            client,
            keep_alive: Box::new((event_loop, http_server)),
        },
    })
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
        rpc: jsonrpc_core::MetaIoHandler<Metadata, informant::Middleware<rpc_apis::ClientNotifier>>,
        client: Arc<Client>,
        keep_alive: Box<Any>,
    },
}

impl RunningClient {
    /// Performs a synchronous RPC query.
    /// Blocks execution until the result is ready.
    pub fn rpc_query_sync(&self, request: &str) -> Option<String> {
        let metadata = Metadata {
            origin: Origin::CApi,
            session: None,
        };

        match self.inner {
            RunningClientInner::Full { ref rpc, .. } => rpc.handle_request_sync(request, metadata),
        }
    }

    /// Shuts down the client.
    pub fn shutdown(self) {
        match self.inner {
            RunningClientInner::Full {
                rpc,
                client,
                keep_alive,
            } => {
                info!("Finishing work, please wait...");
                // Create a weak reference to the client so that we can wait on shutdown
                // until it is dropped
                let weak_client = Arc::downgrade(&client);
                // drop this stuff as soon as exit detected.
                drop(rpc);
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
