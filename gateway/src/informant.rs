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

// Based on parity/rpc/src/v1/informant.rs [v1.12.0]

//! RPC Requests Statistics

use std::collections::HashMap;
use std::fmt;
use std::time;

use parity_rpc::v1::types::H256;
use parking_lot::RwLock;

const RATE_SECONDS: usize = 10;

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

/// RPC Statistics
#[derive(Default, Debug)]
pub struct RpcStats {
    requests: RwLock<RateCalculator>,
    sessions: RwLock<HashMap<H256, RwLock<RateCalculator>>>,
}

impl RpcStats {
    /// Start tracking a session
    pub fn open_session(&self, id: H256) {
        self.sessions
            .write()
            .insert(id, RwLock::new(RateCalculator::default()));
    }

    /// Stop tracking a session
    pub fn close_session(&self, id: &H256) {
        self.sessions.write().remove(id);
    }

    /// Count request. Returns number of requests in current second.
    pub fn count_request(&self, id: &H256) -> u16 {
        self.sessions
            .read()
            .get(id)
            .map(|calc| calc.write().tick())
            .unwrap_or(0)
    }

    /// Returns number of open sessions
    pub fn sessions(&self) -> usize {
        self.sessions.read().len()
    }

    /// Returns requests rate
    pub fn requests_rate(&self, id: &H256) -> usize {
        self.sessions
            .read()
            .get(id)
            .map(|calc| calc.read().rate())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {

    use super::{H256, RateCalculator, RpcStats};

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
    fn should_count_rpc_stats() {
        // given
        let stats = RpcStats::default();
        assert_eq!(stats.sessions(), 0);
        assert_eq!(stats.requests_rate(&H256::from(1)), 0);

        // when
        stats.open_session(H256::from(1));
        stats.close_session(&H256::from(1));
        stats.open_session(H256::from(2));
        stats.count_request(&H256::from(2));
        stats.count_request(&H256::from(2));

        // then
        assert_eq!(stats.sessions(), 1);
        assert_eq!(stats.requests_rate(&H256::from(2)), 2);
    }

    #[test]
    fn should_be_sync_and_send() {
        let stats = RpcStats::default();
        is_sync(stats);
    }

    fn is_sync<F: Send + Sync>(x: F) {
        drop(x)
    }
}
