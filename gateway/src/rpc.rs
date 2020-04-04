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

use std::{collections::HashSet, io, sync::Arc};

use informant::RpcStats;
use jsonrpc_core::MetaIoHandler;
use jsonrpc_http_server::tokio::runtime::TaskExecutor;
use middleware::{Middleware, WsDispatcher, WsStats};
use parity_rpc::{self as rpc, DomainsValidation, Metadata};
use rpc_apis::{self, ApiSet};

use servers;

pub use parity_rpc::{ws::Server as WsServer, HttpServer, RequestMiddleware};

#[derive(Debug, Clone, PartialEq)]
pub struct HttpConfiguration {
    pub enabled: bool,
    pub interface: String,
    pub port: u16,
    pub apis: ApiSet,
    pub cors: Option<Vec<String>>,
    pub hosts: Option<Vec<String>>,
    pub server_threads: usize,
    pub max_batch_size: usize,
}

impl Default for HttpConfiguration {
    fn default() -> Self {
        HttpConfiguration {
            enabled: true,
            interface: "127.0.0.1".into(),
            port: 8545,
            apis: ApiSet::UnsafeContext,
            cors: Some(vec![]),
            hosts: Some(vec![]),
            server_threads: 1,
            max_batch_size: 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WsConfiguration {
    pub enabled: bool,
    pub interface: String,
    pub port: u16,
    pub apis: ApiSet,
    pub max_connections: usize,
    pub origins: Option<Vec<String>>,
    pub hosts: Option<Vec<String>>,
    pub support_token_api: bool,
    pub dapps_address: Option<rpc::Host>,
    pub max_batch_size: usize,
    pub max_req_per_sec: usize,
}

impl Default for WsConfiguration {
    fn default() -> Self {
        WsConfiguration {
            enabled: true,
            interface: "127.0.0.1".into(),
            port: 8546,
            apis: ApiSet::UnsafeContext,
            max_connections: 100,
            origins: Some(vec![
                "parity://*".into(),
                "chrome-extension://*".into(),
                "moz-extension://*".into(),
            ]),
            hosts: Some(Vec::new()),
            support_token_api: true,
            dapps_address: Some("127.0.0.1:8545".into()),
            max_batch_size: 10,
            max_req_per_sec: 50,
        }
    }
}

impl WsConfiguration {
    pub fn address(&self) -> Option<rpc::Host> {
        address(self.enabled, &self.interface, self.port, &self.hosts)
    }
}

fn address(
    enabled: bool,
    bind_iface: &str,
    bind_port: u16,
    hosts: &Option<Vec<String>>,
) -> Option<rpc::Host> {
    if !enabled {
        return None;
    }

    match *hosts {
        Some(ref hosts) if !hosts.is_empty() => Some(hosts[0].clone().into()),
        _ => Some(format!("{}:{}", bind_iface, bind_port).into()),
    }
}

pub struct Dependencies<D: rpc_apis::Dependencies> {
    pub apis: Arc<D>,
    pub executor: TaskExecutor,
    pub stats: Arc<RpcStats>,
}

pub fn new_ws<D: rpc_apis::Dependencies>(
    conf: WsConfiguration,
    deps: &Dependencies<D>,
) -> Result<Option<WsServer>, String> {
    if !conf.enabled {
        return Ok(None);
    }

    let url = format!("{}:{}", conf.interface, conf.port);
    let addr = url
        .parse()
        .map_err(|_| format!("Invalid WebSockets listen host/port given: {}", url))?;

    let handler = {
        let mut handler = MetaIoHandler::with_middleware((
            WsDispatcher::new(deps.stats.clone(), conf.max_req_per_sec),
            Middleware::new(deps.apis.activity_notifier(), conf.max_batch_size),
        ));
        let apis = conf.apis.list_apis();
        deps.apis.extend_with_set(&mut handler, &apis);

        handler
    };

    let remote = deps.executor.clone();
    let allowed_origins = into_domains(collect_hosts(conf.origins, &conf.dapps_address));
    let allowed_hosts = into_domains(collect_hosts(conf.hosts, &Some(url.clone().into())));

    let start_result = servers::start_ws(
        &addr,
        handler,
        remote.clone(),
        allowed_origins,
        allowed_hosts,
        conf.max_connections,
        rpc::WsExtractor::new(None),
        rpc::WsExtractor::new(None),
        WsStats::new(deps.stats.clone()),
    );

    match start_result {
        Ok(server) => Ok(Some(server)),
        Err(rpc::ws::Error(rpc::ws::ErrorKind::Io(ref err), _))
            if err.kind() == io::ErrorKind::AddrInUse => Err(
            format!("WebSockets address {} is already in use, make sure that another instance of an Ethereum client is not running or change the address using the --ws-port and --ws-interface options.", url)
        ),
        Err(e) => Err(format!("WebSockets error: {:?}", e)),
    }
}

pub fn new_http<D: rpc_apis::Dependencies>(
    id: &str,
    options: &str,
    conf: HttpConfiguration,
    deps: &Dependencies<D>,
) -> Result<Option<HttpServer>, String> {
    if !conf.enabled {
        return Ok(None);
    }

    let url = format!("{}:{}", conf.interface, conf.port);
    let addr = url
        .parse()
        .map_err(|_| format!("Invalid {} listen host/port given: {}", id, url))?;
    let handler = setup_apis(conf.apis, deps, conf.max_batch_size);
    let executor = deps.executor.clone();

    let cors_domains = into_domains(conf.cors);
    let allowed_hosts = into_domains(collect_hosts(conf.hosts, &Some(url.clone().into())));

    let start_result = servers::start_http(
        &addr,
        cors_domains,
        allowed_hosts,
        handler,
        executor,
        rpc::RpcExtractor,
        conf.server_threads,
    );

    match start_result {
        Ok(server) => Ok(Some(server)),
        Err(ref err) if err.kind() == io::ErrorKind::AddrInUse => Err(format!(
            "{} address {} is already in use, make sure that another instance \
             of an Ethereum client is not running or change the address using \
             the --{}-port and --{}-interface options.",
            id, url, options, options
        )),
        Err(e) => Err(format!("{} error: {:?}", id, e)),
    }
}

fn into_domains<T: From<String>>(items: Option<Vec<String>>) -> DomainsValidation<T> {
    items
        .map(|vals| vals.into_iter().map(T::from).collect())
        .into()
}

fn collect_hosts(
    items: Option<Vec<String>>,
    dapps_address: &Option<rpc::Host>,
) -> Option<Vec<String>> {
    items.map(move |items| {
        let mut items = items.into_iter().collect::<HashSet<_>>();
        {
            let mut add_hosts = |address: &Option<rpc::Host>| {
                if let Some(host) = address.clone() {
                    items.insert(host.to_string());
                    items.insert(host.replace("127.0.0.1", "localhost"));
                }
            };

            add_hosts(dapps_address);
        }
        items.into_iter().collect()
    })
}

pub fn setup_apis<D>(
    apis: ApiSet,
    deps: &Dependencies<D>,
    max_batch_size: usize,
) -> MetaIoHandler<Metadata, Middleware<D::Notifier>>
where
    D: rpc_apis::Dependencies,
{
    let mut handler = MetaIoHandler::with_middleware(Middleware::new(
        deps.apis.activity_notifier(),
        max_batch_size,
    ));
    let apis = apis.list_apis();
    deps.apis.extend_with_set(&mut handler, &apis);

    handler
}

#[cfg(test)]
mod tests {
    use super::address;

    #[test]
    fn should_return_proper_address() {
        assert_eq!(address(false, "localhost", 8180, &None), None);
        assert_eq!(
            address(true, "localhost", 8180, &None),
            Some("localhost:8180".into())
        );
        assert_eq!(
            address(true, "localhost", 8180, &Some(vec!["host:443".into()])),
            Some("host:443".into())
        );
        assert_eq!(
            address(true, "localhost", 8180, &Some(vec!["host".into()])),
            Some("host".into())
        );
    }
}
