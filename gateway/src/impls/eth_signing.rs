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

use ethereum_types::H256;

use jsonrpc_core::futures::future::Either;
use jsonrpc_core::futures::{future, Future};
use jsonrpc_core::{BoxFuture, Result};
use parity_rpc::v1::helpers::errors;
use parity_rpc::v1::metadata::Metadata;
use parity_rpc::v1::traits::EthSigning;
use parity_rpc::v1::types::{Bytes, H160 as RpcH160, H256 as RpcH256, H520 as RpcH520,
                            RichRawTransaction, TransactionRequest as RpcTransactionRequest};

use evm_api::TransactionRequest;

pub struct EthSigningClient {
    client: Arc<Client>,
}

impl EthSigningClient {
    pub fn new(client: Arc<Client>) -> Self {
        EthSigningClient { client: client }
    }
}

impl EthSigning for EthSigningClient {
    type Metadata = Metadata;

    fn sign(&self, meta: Metadata, address: RpcH160, data: Bytes) -> BoxFuture<RpcH520> {
        Box::new(future::err(errors::unimplemented(None)))
    }

    fn send_transaction(
        &self,
        meta: Metadata,
        request: RpcTransactionRequest,
    ) -> BoxFuture<RpcH256> {
        let request = TransactionRequest {
            nonce: request.nonce.map(Into::into),
            caller: request.from.map(Into::into),
            is_call: request.to.is_some(),
            address: request.to.map(Into::into),
            input: request.data.map(Into::into),
            value: request.value.map(Into::into),
        };

        let result = self.client.send_transaction(request);
        Box::new(future::done(result.map_err(errors::call).map(Into::into)))
    }

    fn sign_transaction(
        &self,
        meta: Metadata,
        request: RpcTransactionRequest,
    ) -> BoxFuture<RichRawTransaction> {
        Box::new(future::err(errors::unimplemented(None)))
    }
}
