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

//! Eth Filter RPC implementation

use std::sync::Arc;

use client::Client;
use util::log_to_rpc_log;

use ethcore::client::BlockId;
use ethcore::filter::Filter as EthcoreFilter;
use ethereum_types::H256;
use parking_lot::Mutex;

use jsonrpc_core::futures::future;
use jsonrpc_core::BoxFuture;
use parity_rpc::v1::helpers::{PollFilter, PollManager};
use parity_rpc::v1::impls::eth_filter::Filterable;
use parity_rpc::v1::types::{H256 as RpcH256, Log as RpcLog};

/// Eth filter rpc implementation for a full node.
pub struct EthFilterClient {
    client: Arc<Client>,
    polls: Mutex<PollManager<PollFilter>>,
}

impl EthFilterClient {
    /// Creates new Eth filter client.
    pub fn new(client: Arc<Client>) -> Self {
        EthFilterClient {
            client: client,
            polls: Mutex::new(PollManager::new()),
        }
    }
}

impl Filterable for EthFilterClient {
    fn best_block_number(&self) -> u64 {
        self.client.best_block_number()
    }

    fn block_hash(&self, id: BlockId) -> Option<RpcH256> {
        self.client.block_hash(id).map(Into::into)
    }

    fn pending_transactions_hashes(&self) -> Vec<H256> {
        Vec::new()
    }

    #[cfg(feature = "read_state")]
    fn logs(&self, filter: EthcoreFilter) -> BoxFuture<Vec<RpcLog>> {
        measure_counter_inc!("getFilterLogs");
        info!("eth_getFilterLogs(filter: {:?})", filter);
        Box::new(future::ok({
            self.client
                .logs(filter)
                .into_iter()
                .map(From::from)
                .collect::<Vec<RpcLog>>()
        }))
    }

    #[cfg(not(feature = "read_state"))]
    fn logs(&self, filter: EthcoreFilter) -> BoxFuture<Vec<RpcLog>> {
        measure_counter_inc!("getFilterLogs");
        info!("eth_getFilterLogs(filter: {:?})", filter);
        Box::new(future::ok({
            self.client
                .logs(filter)
                .into_iter()
                .map(log_to_rpc_log)
                .collect()
        }))
    }

    fn pending_logs(&self, _block_number: u64, _filter: &EthcoreFilter) -> Vec<RpcLog> {
        Vec::new()
    }

    fn polls(&self) -> &Mutex<PollManager<PollFilter>> {
        &self.polls
    }
}
