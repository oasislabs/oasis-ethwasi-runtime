//! RPC Middleware

use informant::RpcStats;
use jsonrpc_core as rpc;
use jsonrpc_ws_server as ws;
use parity_rpc::informant::ActivityNotifier;
use parity_rpc::v1::types::H256;
use parity_rpc::{Metadata, Origin};
use std::sync::Arc;
use std::time;

/// Custom JSON-RPC error codes
const ERROR_BATCH_SIZE: i64 = -32099;
const ERROR_RATE_LIMITED: i64 = -32098;

/// A custom JSON-RPC error for batches containing too many requests.
fn error_batch_size() -> rpc::Error {
    rpc::Error {
        code: rpc::ErrorCode::ServerError(ERROR_BATCH_SIZE),
        message: "Too many JSON-RPC requests in batch".into(),
        data: None,
    }
}

/// A custom JSON-RPC error for WebSocket rate limit.
fn error_rate_limited() -> rpc::Error {
    rpc::Error {
        code: rpc::ErrorCode::ServerError(ERROR_RATE_LIMITED),
        message: "Too many requests".into(),
        data: None,
    }
}

/// RPC middleware that enforces batch size limits.
pub struct Middleware<T: ActivityNotifier> {
    notifier: T,
    max_batch_size: usize,
}

impl<T: ActivityNotifier> Middleware<T> {
    pub fn new(notifier: T, max_batch_size: usize) -> Self {
        Middleware {
            notifier,
            max_batch_size,
        }
    }
}

impl<M: rpc::Metadata, T: ActivityNotifier> rpc::Middleware<M> for Middleware<T> {
    type Future = rpc::FutureResponse;

    fn on_request<F, X>(&self, request: rpc::Request, meta: M, process: F) -> Self::Future
    where
        F: FnOnce(rpc::Request, M) -> X,
        X: rpc::futures::Future<Item = Option<rpc::Response>, Error = ()> + Send + 'static,
    {
        self.notifier.active();

        // Check the number of requests in the JSON-RPC batch.
        if let rpc::Request::Batch(ref calls) = request {
            let batch_size = calls.len();
            measure_histogram!("jsonrpc_batch_size", batch_size);

            // If it exceeds the limit, respond with a custom application error.
            if batch_size > self.max_batch_size {
                error!("Rejecting JSON-RPC batch: {:?} requests", batch_size);
                return Box::new(rpc::futures::finished(Some(rpc::Response::from(
                    error_batch_size(),
                    None,
                ))));
            }
        }

        Box::new(process(request, meta))
    }
}

/// WebSockets middleware that dispatches requests to handle.
pub struct WsDispatcher<M: rpc::Middleware<Metadata>> {
    full_handler: rpc::MetaIoHandler<Metadata, M>,
    stats: Arc<RpcStats>,
    max_rate: usize,
}

impl<M: rpc::Middleware<Metadata>> WsDispatcher<M> {
    /// Create new `WsDispatcher` with given full handler.
    pub fn new(
        full_handler: rpc::MetaIoHandler<Metadata, M>,
        stats: Arc<RpcStats>,
        max_rate: usize,
    ) -> Self {
        WsDispatcher {
            full_handler: full_handler,
            stats: stats,
            max_rate: max_rate,
        }
    }
}

impl<M: rpc::Middleware<Metadata>> rpc::Middleware<Metadata> for WsDispatcher<M> {
    type Future = rpc::FutureResponse;

    fn on_request<F, X>(&self, request: rpc::Request, meta: Metadata, process: F) -> Self::Future
    where
        F: FnOnce(rpc::Request, Metadata) -> X,
        X: rpc::futures::Future<Item = Option<rpc::Response>, Error = ()> + Send + 'static,
    {
        match meta.origin {
            Origin::Ws {
                ref session,
                dapp: _,
            } => {
                // TODO: max request rate parameter
                if self.stats.count_request(session) > self.max_rate as u16 {
                    error!("Rejecting WS request");
                    return Box::new(rpc::futures::finished(Some(rpc::Response::from(
                        error_rate_limited(),
                        None,
                    ))));
                }
            }
            _ => (),
        };

        Box::new(process(request, meta))
    }
}

/// WebSockets RPC usage statistics.
pub struct WsStats {
    stats: Arc<RpcStats>,
}

impl WsStats {
    /// Creates new WS usage tracker.
    pub fn new(stats: Arc<RpcStats>) -> Self {
        WsStats { stats: stats }
    }
}

impl ws::SessionStats for WsStats {
    fn open_session(&self, id: ws::SessionId) {
        self.stats.open_session(H256::from(id))
    }

    fn close_session(&self, id: ws::SessionId) {
        self.stats.close_session(&H256::from(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub struct TestNotifier {}

    impl ActivityNotifier for TestNotifier {
        fn active(&self) {}
    }

    #[test]
    fn should_limit_batch_size() {
        use futures::Future;
        use jsonrpc_core::Middleware as mw;

        // Middleware that accepts a max batch size of 1 request
        let middleware = Middleware::new(TestNotifier {}, 1);

        let batch_1 = rpc::Request::Batch(vec![rpc::Call::MethodCall(rpc::MethodCall {
            jsonrpc: Some(rpc::Version::V2),
            method: "test".to_owned(),
            params: Some(rpc::Params::Array(vec![
                rpc::Value::from(1),
                rpc::Value::from(2),
            ])),
            id: rpc::Id::Num(1),
        })]);

        let batch_2 = rpc::Request::Batch(vec![
            rpc::Call::MethodCall(rpc::MethodCall {
                jsonrpc: Some(rpc::Version::V2),
                method: "test".to_owned(),
                params: Some(rpc::Params::Array(vec![
                    rpc::Value::from(1),
                    rpc::Value::from(2),
                ])),
                id: rpc::Id::Num(2),
            }),
            rpc::Call::Notification(rpc::Notification {
                jsonrpc: Some(rpc::Version::V2),
                method: "test".to_owned(),
                params: Some(rpc::Params::Array(vec![rpc::Value::from(1)])),
            }),
        ]);

        // batch size: 1 (should pass)
        let response_1 = middleware
            .on_request(batch_1, (), |request, meta| {
                Box::new(rpc::futures::finished(None))
            })
            .wait()
            .unwrap();

        // no Failure response for batch size of 1
        assert_eq!(response_1, None);

        // batch size: 2 (should fail)
        let response_2 = middleware
            .on_request(batch_2, (), |request, meta| {
                Box::new(rpc::futures::finished(None))
            })
            .wait()
            .unwrap();

        // should respond with a Failure for batch size of 2
        match response_2 {
            Some(rpc::Response::Single(rpc::Output::Failure(failure))) => {
                assert_eq!(
                    failure.error.code,
                    rpc::ErrorCode::ServerError(ERROR_BATCH_SIZE)
                );
            }
            _ => assert!(false, "Did not enforce batch size limit"),
        };
    }
}
