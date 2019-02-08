//! RPC Middleware

use informant::RpcStats;
use jsonrpc_core as rpc;
use jsonrpc_ws_server as ws;
use parity_rpc::{informant::ActivityNotifier, v1::types::H256, Metadata, Origin};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::vec::Vec;

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

pub struct Method {
    method: String,
    id: String,
}

pub struct Notification {
    method: String,
}

pub struct Invalid {
    id: String,
}

pub enum RequestType {
    Method(Method),
    Notification(Notification),
    Invalid(Invalid),
}

pub enum RequestData {
    Single(RequestType),
    Batch(Vec<RequestType>),
}

impl RequestData {
    pub fn from_request(request: &rpc::Request) -> RequestData {
        match request {
            rpc::Request::Single(call) => RequestData::Single(RequestType::from_call(&call)),
            rpc::Request::Batch(calls) => RequestData::Batch(
                calls
                    .iter()
                    .map(|ref call| RequestType::from_call(&call))
                    .collect::<Vec<_>>(),
            ),
        }
    }
}

impl RequestType {
    fn id_to_string(id: &rpc::Id) -> String {
        match id {
            rpc::Id::Null => String::from("null"),
            rpc::Id::Str(s) => s.clone(),
            rpc::Id::Num(n) => n.to_string(),
        }
    }

    pub fn from_call(call: &rpc::Call) -> RequestType {
        match call {
            rpc::Call::MethodCall(method) => RequestType::Method(Method {
                method: method.method.clone(),
                id: RequestType::id_to_string(&method.id),
            }),
            rpc::Call::Notification(notification) => RequestType::Notification(Notification {
                method: notification.method.clone(),
            }),
            rpc::Call::Invalid(id) => RequestType::Invalid(Invalid {
                id: RequestType::id_to_string(id),
            }),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            RequestType::Method(method) => format!(
                "method: {:?}, type: method, id: {:?}",
                method.method, method.id
            ),
            RequestType::Notification(notification) => {
                format!("method: {:?}, type: notification", notification.method)
            }
            RequestType::Invalid(invalid) => format!("type: invalid, id: {:?}", invalid.id),
        }
    }
}

/// RequestLogger middleware that logs relevant
/// information about the requests received
pub struct RequestLogger {}

impl RequestLogger {
    pub fn new() -> Self {
        return RequestLogger {};
    }

    pub fn log_call(rt: &RequestType, out: &rpc::Output, start: std::time::Instant) {
        let duration = start.elapsed().as_millis();

        let ok = match out {
            rpc::Output::Success(_) => true,
            rpc::Output::Failure(_) => false,
        };

        info!(
            "callType: HandleRequest, duration: {:?}, success: {:?}, {:?}",
            duration,
            ok,
            rt.to_string()
        );
    }

    pub fn log_calls(rts: &Vec<RequestType>, response: &rpc::Response, start: std::time::Instant) {
        match response {
            rpc::Response::Single(output) => panic!("batch call with single response"),
            rpc::Response::Batch(outputs) => {
                for (i, rt) in rts.iter().enumerate() {
                    RequestLogger::log_call(rt, &outputs[i], start);
                }
            }
        }
    }

    pub fn log_ok(data: &RequestData, response: &rpc::Response, start: std::time::Instant) {
        match data {
            RequestData::Single(rt) => match response {
                rpc::Response::Single(output) => RequestLogger::log_call(&rt, &output, start),
                rpc::Response::Batch(output) => panic!("single call with batch response"),
            },
            RequestData::Batch(rts) => RequestLogger::log_calls(rts, response, start),
        }
    }

    pub fn log_empty_call(rt: &RequestType, start: std::time::Instant, result: &str) {
        let duration = start.elapsed().as_millis();

        info!(
            "callType: HandleRequest, duration: {:?}, {:?}",
            duration,
            rt.to_string()
        );
    }

    pub fn log_empty_calls(rts: &Vec<RequestType>, start: std::time::Instant, result: &str) {
        for rt in rts.iter() {
            RequestLogger::log_empty_call(rt, start, result);
        }
    }

    pub fn log_empty(data: &RequestData, start: std::time::Instant) {
        match data {
            RequestData::Single(rt) => RequestLogger::log_empty_call(rt, start, "null"),
            RequestData::Batch(rts) => RequestLogger::log_empty_calls(rts, start, "null"),
        }
    }

    pub fn log_error(data: &RequestData, start: std::time::Instant) {
        match data {
            RequestData::Single(rt) => RequestLogger::log_empty_call(rt, start, "error"),
            RequestData::Batch(rts) => RequestLogger::log_empty_calls(rts, start, "error"),
        }
    }
}

impl rpc::Middleware<Metadata> for RequestLogger {
    type Future = rpc::FutureResponse;

    fn on_request<F, X>(&self, request: rpc::Request, meta: Metadata, process: F) -> Self::Future
    where
        F: FnOnce(rpc::Request, Metadata) -> X,
        X: rpc::futures::Future<Item = Option<rpc::Response>, Error = ()> + Send + 'static,
    {
        let data = RequestData::from_request(&request);
        let now = Instant::now();
        let future = process(request, meta);

        Box::new(future.then(move |result| match result {
            Ok(opt) => match opt {
                Some(response) => {
                    RequestLogger::log_ok(&data, &response, now);
                    Ok(Some(response))
                }
                None => {
                    RequestLogger::log_empty(&data, now);
                    Ok(None)
                }
            },
            Err(_) => {
                RequestLogger::log_error(&data, now);
                Err(())
            }
        }))
    }
}

/// WebSockets middleware that dispatches requests to handle.
pub struct WsDispatcher {
    stats: Arc<RpcStats>,
    max_req_per_sec: usize,
}

impl WsDispatcher {
    /// Create new `WsDispatcher` with given full handler.
    pub fn new(stats: Arc<RpcStats>, max_req_per_sec: usize) -> Self {
        WsDispatcher {
            stats: stats,
            max_req_per_sec: max_req_per_sec,
        }
    }
}

impl rpc::Middleware<Metadata> for WsDispatcher {
    type Future = rpc::FutureResponse;

    fn on_request<F, X>(&self, request: rpc::Request, meta: Metadata, process: F) -> Self::Future
    where
        F: FnOnce(rpc::Request, Metadata) -> X,
        X: rpc::futures::Future<Item = Option<rpc::Response>, Error = ()> + Send + 'static,
    {
        // Check request rate for session, and respond with an error if it exceeds max_req_per_sec.
        match meta.origin {
            Origin::Ws {
                ref session,
                dapp: _,
            } => {
                if self.stats.count_request(session) as usize > self.max_req_per_sec {
                    measure_counter_inc!("ws_rate_limited");
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

    use futures::Future;
    use informant::RpcStats;
    use jsonrpc_core::Middleware as mw;

    pub struct TestNotifier {}

    impl ActivityNotifier for TestNotifier {
        fn active(&self) {}
    }

    fn make_request(id: u64) -> rpc::Request {
        rpc::Request::Single(rpc::Call::MethodCall(rpc::MethodCall {
            jsonrpc: Some(rpc::Version::V2),
            method: "test".to_owned(),
            params: Some(rpc::Params::Array(vec![
                rpc::Value::from(1),
                rpc::Value::from(2),
            ])),
            id: rpc::Id::Num(id),
        }))
    }

    #[test]
    fn should_not_limit_request_rate() {
        let stats = Arc::new(RpcStats::default());

        // start a new WS session
        let session_id = H256::from(1);
        stats.open_session(session_id.clone());
        let metadata = Metadata {
            origin: Origin::Ws {
                dapp: "".into(),
                session: session_id.clone(),
            },
            session: None,
        };

        // limit: 1 request/sec
        let dispatcher = WsDispatcher::new(stats.clone(), 1);

        // a single request (should pass)
        let request_1 = make_request(1);

        let response = dispatcher
            .on_request(request_1, metadata.clone(), |request, meta| {
                Box::new(rpc::futures::finished(None))
            })
            .wait()
            .unwrap();

        // no Failure response for a single request
        assert_eq!(response, None);
    }

    #[test]
    fn should_limit_request_rate() {
        let stats = Arc::new(RpcStats::default());

        // start a new WS session
        let session_id = H256::from(1);
        stats.open_session(session_id.clone());
        let metadata = Metadata {
            origin: Origin::Ws {
                dapp: "".into(),
                session: session_id.clone(),
            },
            session: None,
        };

        // limit: 1 request/sec
        let dispatcher = WsDispatcher::new(stats.clone(), 1);

        // two requests
        let request_1 = make_request(1);
        let request_2 = make_request(2);

        let response = dispatcher
            .on_request(request_1, metadata.clone(), |request, meta| {
                Box::new(rpc::futures::finished(None))
            })
            .wait()
            .unwrap();

        let response = dispatcher
            .on_request(request_2, metadata.clone(), |request, meta| {
                Box::new(rpc::futures::finished(None))
            })
            .wait()
            .unwrap();

        // should respond with a Failure
        match response {
            Some(rpc::Response::Single(rpc::Output::Failure(failure))) => {
                assert_eq!(
                    failure.error.code,
                    rpc::ErrorCode::ServerError(ERROR_RATE_LIMITED)
                );
            }
            _ => assert!(false, "Did not enforce rate limit"),
        };
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
