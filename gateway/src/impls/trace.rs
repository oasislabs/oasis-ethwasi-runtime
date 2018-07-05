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

//! Traces api implementation.

use jsonrpc_core::Result;
use jsonrpc_macros::Trailing;
use parity_rpc::v1::helpers::errors;
use parity_rpc::v1::traits::Traces;
use parity_rpc::v1::types::{BlockNumber, Bytes, CallRequest, H256, Index, LocalizedTrace,
                            TraceFilter, TraceOptions, TraceResults};
use parity_rpc::v1::Metadata;

/// Traces api implementation.
pub struct TracesClient;

impl TracesClient {
    /// Creates new TracesClient.
    pub fn new() -> Self {
        TracesClient
    }
}

impl Traces for TracesClient {
    type Metadata = Metadata;

    fn filter(&self, _filter: TraceFilter) -> Result<Option<Vec<LocalizedTrace>>> {
        Err(errors::unimplemented(None))
    }

    fn block_traces(&self, _block_number: BlockNumber) -> Result<Option<Vec<LocalizedTrace>>> {
        Err(errors::unimplemented(None))
    }

    fn transaction_traces(&self, _transaction_hash: H256) -> Result<Option<Vec<LocalizedTrace>>> {
        Err(errors::unimplemented(None))
    }

    fn trace(
        &self,
        _transaction_hash: H256,
        _address: Vec<Index>,
    ) -> Result<Option<LocalizedTrace>> {
        Err(errors::unimplemented(None))
    }

    fn call(
        &self,
        _meta: Self::Metadata,
        _request: CallRequest,
        _flags: TraceOptions,
        _block: Trailing<BlockNumber>,
    ) -> Result<TraceResults> {
        Err(errors::unimplemented(None))
    }

    fn call_many(
        &self,
        _meta: Self::Metadata,
        _request: Vec<(CallRequest, TraceOptions)>,
        _block: Trailing<BlockNumber>,
    ) -> Result<Vec<TraceResults>> {
        Err(errors::unimplemented(None))
    }

    fn raw_transaction(
        &self,
        _raw_transaction: Bytes,
        _flags: TraceOptions,
        _block: Trailing<BlockNumber>,
    ) -> Result<TraceResults> {
        Err(errors::unimplemented(None))
    }

    fn replay_transaction(
        &self,
        _transaction_hash: H256,
        _flags: TraceOptions,
    ) -> Result<TraceResults> {
        Err(errors::unimplemented(None))
    }

    fn replay_block_transactions(
        &self,
        _block_number: BlockNumber,
        _flags: TraceOptions,
    ) -> Result<Vec<TraceResults>> {
        Err(errors::unimplemented(None))
    }
}
