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

use std::collections::HashSet;
use std::sync::Arc;

use client::Client;

use ethcore::client::{BlockChainClient, BlockId};
use ethcore::filter::Filter as EthcoreFilter;
use ethcore::miner;
use ethereum_types::H256;
use parking_lot::Mutex;

use jsonrpc_core::futures::future::Either;
use jsonrpc_core::futures::{future, Future};
use jsonrpc_core::{BoxFuture, Result};
use parity_rpc::v1::helpers::{errors, limit_logs, PollFilter, PollManager};
use parity_rpc::v1::impls::eth_filter::Filterable;
use parity_rpc::v1::traits::EthFilter;
use parity_rpc::v1::types::{BlockNumber, Filter, FilterChanges, H256 as RpcH256, Index, Log,
                            U256 as RpcU256};

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

    fn logs(&self, filter: EthcoreFilter) -> BoxFuture<Vec<Log>> {
        Box::new(future::ok(
            self.client
                .logs(filter)
                .into_iter()
                .map(Into::into)
                .collect(),
        ))
    }

    fn pending_logs(&self, block_number: u64, filter: &EthcoreFilter) -> Vec<Log> {
        Vec::new()
    }

    fn polls(&self) -> &Mutex<PollManager<PollFilter>> {
        &self.polls
    }
}
