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

//! Parity RPC.

#![warn(missing_docs)]

use jsonrpc_core;
use jsonrpc_http_server::{self as http, hyper, tokio_core};
use jsonrpc_ws_server as ws;

use parity_rpc::http_common::{self, HttpMetaExtractor};

use std::net::SocketAddr;

/// RPC HTTP Server instance
pub type HttpServer = http::Server;

/// Start http server asynchronously and returns result with `Server` handle on success or an error.
pub fn start_http<M, S, H, T>(
    addr: &SocketAddr,
    cors_domains: http::DomainsValidation<http::AccessControlAllowOrigin>,
    allowed_hosts: http::DomainsValidation<http::Host>,
    handler: H,
    remote: tokio_core::reactor::Remote,
    extractor: T,
    threads: usize,
) -> ::std::io::Result<HttpServer>
where
    M: jsonrpc_core::Metadata,
    S: jsonrpc_core::Middleware<M>,
    H: Into<jsonrpc_core::MetaIoHandler<M, S>>,
    T: HttpMetaExtractor<Metadata = M>,
{
    let extractor = http_common::MetaExtractor::new(extractor);
    let builder = http::ServerBuilder::with_meta_extractor(handler, extractor)
        .threads(threads)
        .event_loop_remote(remote)
        .request_middleware(|request: hyper::Request<hyper::Body>| {
            // If the requested url is /status, terminate with 200 OK response.
            // Otherwise, proceed with normal request handling.
            if request.uri() == "/status" {
                http::Response::ok("").into()
            } else {
                request.into()
            }
        })
        .cors(cors_domains.into())
        .allowed_hosts(allowed_hosts.into());

    Ok(builder.start_http(addr)?)
}

/// Start WS server and return `Server` handle.
pub fn start_ws<M, S, H, T, U, V>(
    addr: &SocketAddr,
    handler: H,
    remote: tokio_core::reactor::Remote,
    allowed_origins: ws::DomainsValidation<ws::Origin>,
    allowed_hosts: ws::DomainsValidation<ws::Host>,
    max_connections: usize,
    extractor: T,
    middleware: V,
    stats: U,
) -> Result<ws::Server, ws::Error>
where
    M: jsonrpc_core::Metadata,
    S: jsonrpc_core::Middleware<M>,
    H: Into<jsonrpc_core::MetaIoHandler<M, S>>,
    T: ws::MetaExtractor<M>,
    U: ws::SessionStats,
    V: ws::RequestMiddleware,
{
    ws::ServerBuilder::with_meta_extractor(handler, extractor)
        .event_loop_remote(remote)
        .request_middleware(middleware)
        .allowed_origins(allowed_origins)
        .allowed_hosts(allowed_hosts)
        .max_connections(max_connections)
        .session_stats(stats)
        .start(addr)
}
