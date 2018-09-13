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

//! RPC implementations for the client.
//!
//! This doesn't re-implement all of the RPC APIs, just those which aren't
//! significantly generic to be reused.

pub mod eth;
pub mod eth_filter;
pub mod eth_pubsub;
pub mod net;
pub mod oasis;
pub mod trace;
pub mod web3;

pub use self::eth::EthClient;
pub use self::eth_filter::EthFilterClient;
pub use self::eth_pubsub::EthPubSubClient;
pub use self::net::NetClient;
pub use self::oasis::OasisClient;
pub use self::trace::TracesClient;
pub use self::web3::Web3Client;
