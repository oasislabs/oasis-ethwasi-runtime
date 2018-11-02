//! RPC Middleware

use jsonrpc_core as rpc;
use parity_rpc::informant::{ActivityNotifier, RpcStats};
use std::sync::Arc;
use std::time;

/// Custom JSON-RPC error code for oversized batches
const ERROR_BATCH_SIZE: i64 = -32099;

/// RPC middleware that counts stats and enforces batch size limits.
pub struct Middleware<T: ActivityNotifier> {
    stats: Arc<RpcStats>,
    notifier: T,
    max_batch_size: usize,
}

impl<T: ActivityNotifier> Middleware<T> {
    /// Create new Middleware with stats counter and activity notifier.
    pub fn new(stats: Arc<RpcStats>, notifier: T, max_batch_size: usize) -> Self {
        Middleware {
            stats,
            notifier,
            max_batch_size,
        }
    }

    fn as_micro(dur: time::Duration) -> u32 {
        (dur.as_secs() * 1_000_000) as u32 + dur.subsec_nanos() / 1_000
    }
}

/// A custom JSON-RPC error for batches containing too many requests.
fn batch_too_large() -> rpc::Error {
    rpc::Error {
        code: rpc::ErrorCode::ServerError(ERROR_BATCH_SIZE),
        message: "Too many JSON-RPC requests in batch".into(),
        data: None,
    }
}

impl<M: rpc::Metadata, T: ActivityNotifier> rpc::Middleware<M> for Middleware<T> {
    type Future = rpc::FutureResponse;

    fn on_request<F, X>(&self, request: rpc::Request, meta: M, process: F) -> Self::Future
    where
        F: FnOnce(rpc::Request, M) -> X,
        X: rpc::futures::Future<Item = Option<rpc::Response>, Error = ()> + Send + 'static,
    {
        let start = time::Instant::now();

        self.notifier.active();
        self.stats.count_request();

        // Check the number of requests in the JSON-RPC batch.
        if let rpc::Request::Batch(ref calls) = request {
            let batch_size = calls.len();
            measure_histogram!("jsonrpc_batch_size", batch_size);

            // If it exceeds the limit, respond with a custom application error.
            if (batch_size > self.max_batch_size) {
                error!("Rejecting JSON-RPC batch: {:?} requests", batch_size);
                return Box::new(rpc::futures::finished(Some(rpc::Response::from(
                    batch_too_large(),
                    None,
                ))));
            }
        }

        let id = match request {
            rpc::Request::Single(rpc::Call::MethodCall(ref call)) => Some(call.id.clone()),
            _ => None,
        };
        let stats = self.stats.clone();
        let future = process(request, meta).map(move |res| {
            let time = Self::as_micro(start.elapsed());
            if time > 10_000 {
                debug!(target: "rpc", "[{:?}] Took {}ms", id, time / 1_000);
            }
            stats.add_roundtrip(time);
            res
        });

        Box::new(future)
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
        let middleware = Middleware::new(Arc::new(RpcStats::default()), TestNotifier {}, 1);

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
