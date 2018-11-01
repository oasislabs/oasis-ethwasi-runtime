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

//! RPC Requests Statistics

use jsonrpc_core as rpc;
use order_stat;
use parking_lot::RwLock;
use std::fmt;
use std::sync::atomic::{self, AtomicUsize};
use std::sync::Arc;
use std::time;

const RATE_SECONDS: usize = 10;
const STATS_SAMPLES: usize = 60;

/// Custom JSON-RPC error code for oversized batches
const ERROR_BATCH_SIZE: i64 = -32091;

struct RateCalculator {
    era: time::Instant,
    samples: [u16; RATE_SECONDS],
}

impl fmt::Debug for RateCalculator {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{} req/s", self.rate())
    }
}

impl Default for RateCalculator {
    fn default() -> Self {
        RateCalculator {
            era: time::Instant::now(),
            samples: [0; RATE_SECONDS],
        }
    }
}

impl RateCalculator {
    fn elapsed(&self) -> u64 {
        self.era.elapsed().as_secs()
    }

    pub fn tick(&mut self) -> u16 {
        if self.elapsed() >= RATE_SECONDS as u64 {
            self.era = time::Instant::now();
            self.samples[0] = 0;
        }

        let pos = self.elapsed() as usize % RATE_SECONDS;
        let next = (pos + 1) % RATE_SECONDS;
        self.samples[next] = 0;
        self.samples[pos] = self.samples[pos].saturating_add(1);
        self.samples[pos]
    }

    fn current_rate(&self) -> usize {
        let now = match self.elapsed() {
            i if i >= RATE_SECONDS as u64 => RATE_SECONDS,
            i => i as usize + 1,
        };
        let sum: usize = self.samples[0..now].iter().map(|x| *x as usize).sum();
        sum / now
    }

    pub fn rate(&self) -> usize {
        if self.elapsed() > RATE_SECONDS as u64 {
            0
        } else {
            self.current_rate()
        }
    }
}

struct StatsCalculator<T = u32> {
    filled: bool,
    idx: usize,
    samples: [T; STATS_SAMPLES],
}

impl<T: Default + Copy> Default for StatsCalculator<T> {
    fn default() -> Self {
        StatsCalculator {
            filled: false,
            idx: 0,
            samples: [T::default(); STATS_SAMPLES],
        }
    }
}

impl<T: fmt::Display + Default + Copy + Ord> fmt::Debug for StatsCalculator<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "median: {} ms", self.approximated_median())
    }
}

impl<T: Default + Copy + Ord> StatsCalculator<T> {
    pub fn add(&mut self, sample: T) {
        self.idx += 1;
        if self.idx >= STATS_SAMPLES {
            self.filled = true;
            self.idx = 0;
        }

        self.samples[self.idx] = sample;
    }

    /// Returns aproximate of media
    pub fn approximated_median(&self) -> T {
        let mut copy = [T::default(); STATS_SAMPLES];
        copy.copy_from_slice(&self.samples);
        let bound = if self.filled {
            STATS_SAMPLES
        } else {
            self.idx + 1
        };

        let (_, &mut median) = order_stat::median_of_medians(&mut copy[0..bound]);
        median
    }
}

/// RPC Statistics
#[derive(Default, Debug)]
pub struct RpcStats {
    requests: RwLock<RateCalculator>,
    roundtrips: RwLock<StatsCalculator<u32>>,
    active_sessions: AtomicUsize,
}

impl RpcStats {
    /// Count session opened
    pub fn open_session(&self) {
        self.active_sessions.fetch_add(1, atomic::Ordering::SeqCst);
    }

    /// Count session closed.
    /// Silently overflows if closing unopened session.
    pub fn close_session(&self) {
        self.active_sessions.fetch_sub(1, atomic::Ordering::SeqCst);
    }

    /// Count request. Returns number of requests in current second.
    pub fn count_request(&self) -> u16 {
        self.requests.write().tick()
    }

    /// Add roundtrip time (microseconds)
    pub fn add_roundtrip(&self, microseconds: u32) {
        self.roundtrips.write().add(microseconds)
    }

    /// Returns number of open sessions
    pub fn sessions(&self) -> usize {
        self.active_sessions.load(atomic::Ordering::Relaxed)
    }

    /// Returns requests rate
    pub fn requests_rate(&self) -> usize {
        self.requests.read().rate()
    }

    /// Returns approximated roundtrip in microseconds
    pub fn approximated_roundtrip(&self) -> u32 {
        self.roundtrips.read().approximated_median()
    }
}

/// Notifies about RPC activity.
pub trait ActivityNotifier: Send + Sync + 'static {
    /// Activity on RPC interface
    fn active(&self);
}

/// Stats-counting RPC middleware
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
    fn should_calculate_rate() {
        // given
        let mut avg = RateCalculator::default();

        // when
        avg.tick();
        avg.tick();
        avg.tick();
        let rate = avg.rate();

        // then
        assert_eq!(rate, 3usize);
    }

    #[test]
    fn should_approximate_median() {
        // given
        let mut stats = StatsCalculator::default();
        stats.add(5);
        stats.add(100);
        stats.add(3);
        stats.add(15);
        stats.add(20);
        stats.add(6);

        // when
        let median = stats.approximated_median();

        // then
        assert_eq!(median, 5);
    }

    #[test]
    fn should_count_rpc_stats() {
        // given
        let stats = RpcStats::default();
        assert_eq!(stats.sessions(), 0);
        assert_eq!(stats.requests_rate(), 0);
        assert_eq!(stats.approximated_roundtrip(), 0);

        // when
        stats.open_session();
        stats.close_session();
        stats.open_session();
        stats.count_request();
        stats.count_request();
        stats.add_roundtrip(125);

        // then
        assert_eq!(stats.sessions(), 1);
        assert_eq!(stats.requests_rate(), 2);
        assert_eq!(stats.approximated_roundtrip(), 125);
    }

    #[test]
    fn should_be_sync_and_send() {
        let stats = RpcStats::default();
        is_sync(stats);
    }

    fn is_sync<F: Send + Sync>(x: F) {
        drop(x)
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
