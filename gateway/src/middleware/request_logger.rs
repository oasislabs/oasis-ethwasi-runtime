//! RPC Middleware

use jsonrpc_core as rpc;
use jsonrpc_ws_server as ws;
use parity_rpc::{informant::ActivityNotifier, v1::types::H256, Metadata, Origin};
use std::{
    sync::Arc,
    time::{Duration, Instant},
    vec::Vec,
};

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
pub struct RequestLogger {
    enabled: bool,
}

impl RequestLogger {
    pub fn new(enabled: bool) -> Self {
        return RequestLogger { enabled: enabled };
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
        if (!self.enabled) {
            return Box::new(process(request, meta));
        }

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
