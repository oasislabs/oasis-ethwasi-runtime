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

//! Net rpc implementation.
use jsonrpc_core::Result;
use parity_rpc::v1::traits::Net;

/// Net rpc implementation.
pub struct NetClient;

impl NetClient {
    /// Creates new NetClient.
    pub fn new() -> Self {
        NetClient
    }
}

impl Net for NetClient {
    fn version(&self) -> Result<String> {
        measure_counter_inc!("net_version");
        // 0A515 1AB5
        Ok(format!("{}", 0xa515))
    }

    fn peer_count(&self) -> Result<String> {
        measure_counter_inc!("net_peerCount");
        Ok(format!("0x{:x}", 0))
    }

    fn is_listening(&self) -> Result<bool> {
        measure_counter_inc!("net_listening");
        Ok(true)
    }
}
