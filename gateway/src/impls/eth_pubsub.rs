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

//! Eth PUB-SUB rpc implementation.

use std::collections::BTreeMap;
use std::sync::{Arc, Weak};

use jsonrpc_core::futures::{self, Future, IntoFuture};
use jsonrpc_core::{BoxFuture, Error, Result};
use jsonrpc_macros::pubsub::{Sink, Subscriber};
use jsonrpc_macros::Trailing;
use jsonrpc_pubsub::SubscriptionId;

use parity_rpc::v1::helpers::{errors, limit_logs, Subscribers};
use parity_rpc::v1::metadata::Metadata;
use parity_rpc::v1::traits::EthPubSub;
use parity_rpc::v1::types::{pubsub, Log, RichHeader};

use bytes::Bytes;
use ethcore::client::BlockId;
use ethcore::encoded;
use ethcore::filter::Filter as EthFilter;
use ethereum_types::H256;
use parity_reactor::Remote;
use parking_lot::{Mutex, RwLock};

use client::{ChainNotify, Client};

type PubSubClient = Sink<pubsub::Result>;

/// Eth PubSub implementation.
pub struct EthPubSubClient {
    handler: Arc<ChainNotificationHandler>,
    heads_subscribers: Arc<RwLock<Subscribers<PubSubClient>>>,
    logs_subscribers: Arc<RwLock<Subscribers<(PubSubClient, EthFilter)>>>,
}

impl EthPubSubClient {
    /// Creates new `EthPubSubClient`.
    pub fn new(client: Arc<Client>, remote: Remote) -> Self {
        let heads_subscribers = Arc::new(RwLock::new(Subscribers::default()));
        let logs_subscribers = Arc::new(RwLock::new(Subscribers::default()));

        EthPubSubClient {
            handler: Arc::new(ChainNotificationHandler {
                client,
                remote,
                heads_subscribers: heads_subscribers.clone(),
                logs_subscribers: logs_subscribers.clone(),
            }),
            heads_subscribers,
            logs_subscribers,
        }
    }

    /// Creates new `EthPubSubCient` with deterministic subscription ids.
    #[cfg(test)]
    pub fn new_test(client: Arc<Client>, remote: Remote) -> Self {
        let client = Self::new(client, remote);
        *client.heads_subscribers.write() = Subscribers::new_test();
        *client.logs_subscribers.write() = Subscribers::new_test();
        client
    }

    /// Returns a chain notification handler.
    pub fn handler(&self) -> Weak<ChainNotificationHandler> {
        Arc::downgrade(&self.handler)
    }
}

/// PubSub Notification handler.
pub struct ChainNotificationHandler {
    client: Arc<Client>,
    remote: Remote,
    heads_subscribers: Arc<RwLock<Subscribers<PubSubClient>>>,
    logs_subscribers: Arc<RwLock<Subscribers<(PubSubClient, EthFilter)>>>,
}

impl ChainNotificationHandler {
    fn notify(remote: &Remote, subscriber: &PubSubClient, result: pubsub::Result) {
        remote.spawn(
            subscriber
                .notify(Ok(result))
                .map(|_| ())
                .map_err(|e| warn!(target: "rpc", "Unable to send notification: {}", e)),
        );
    }

    fn notify_heads(&self, headers: &[(encoded::Header, BTreeMap<String, String>)]) {
        for subscriber in self.heads_subscribers.read().values() {
            for &(ref header, ref extra_info) in headers {
                Self::notify(
                    &self.remote,
                    subscriber,
                    pubsub::Result::Header(RichHeader {
                        inner: header.into(),
                        extra_info: extra_info.clone(),
                    }),
                );
            }
        }
    }

    fn notify_logs<F, T, Ex>(&self, enacted: &[(H256, Ex)], logs: F)
    where
        F: Fn(EthFilter, &Ex) -> T,
        Ex: Send,
        T: IntoFuture<Item = Vec<Log>, Error = Error>,
        T::Future: Send + 'static,
    {
        for &(ref subscriber, ref filter) in self.logs_subscribers.read().values() {
            let logs = futures::future::join_all(
                enacted
                    .iter()
                    .map(|&(hash, ref ex)| {
                        let mut filter = filter.clone();
                        filter.from_block = BlockId::Hash(hash);
                        filter.to_block = filter.from_block.clone();
                        logs(filter, ex).into_future()
                    })
                    .collect::<Vec<_>>(),
            );
            let limit = filter.limit;
            let remote = self.remote.clone();
            let subscriber = subscriber.clone();
            self.remote.spawn(
                logs.map(move |logs| {
                    let logs = logs.into_iter().flat_map(|log| log).collect();

                    for log in limit_logs(logs, limit) {
                        Self::notify(&remote, &subscriber, pubsub::Result::Log(log))
                    }
                }).map_err(|e| warn!("Unable to fetch latest logs: {:?}", e)),
            );
        }
    }
}

impl ChainNotify for ChainNotificationHandler {
    fn new_headers(&self, enacted: &[H256]) {
        // TODO: fix this implementation
        /*
        let headers = enacted
			.iter()
			.filter_map(|hash| self.client.block_header(BlockId::Hash(*hash)))
			.map(|header| (header, Default::default()))
			.collect::<Vec<_>>();

		self.notify_heads(&headers);
		self.notify_logs(&enacted.iter().map(|h| (*h, ())).collect::<Vec<_>>(), |filter, _| self.client.logs(filter))
        */
    }
}

impl EthPubSub for EthPubSubClient {
    type Metadata = Metadata;

    fn subscribe(
        &self,
        _meta: Metadata,
        subscriber: Subscriber<pubsub::Result>,
        kind: pubsub::Kind,
        params: Trailing<pubsub::Params>,
    ) {
        let error = match (kind, params.into()) {
            (pubsub::Kind::NewHeads, None) => {
                self.heads_subscribers.write().push(subscriber);
                return;
            }
            (pubsub::Kind::NewHeads, _) => {
                errors::invalid_params("newHeads", "Expected no parameters.")
            }
            (pubsub::Kind::Logs, Some(pubsub::Params::Logs(filter))) => {
                self.logs_subscribers
                    .write()
                    .push(subscriber, filter.into());
                return;
            }
            (pubsub::Kind::Logs, _) => errors::invalid_params("logs", "Expected a filter object."),
            (pubsub::Kind::NewPendingTransactions, None) => {
                // this is a no-op: we're not mining, so we have no pending transactions
                return;
            }
            (pubsub::Kind::NewPendingTransactions, _) => {
                errors::invalid_params("newPendingTransactions", "Expected no parameters.")
            }
            _ => errors::unimplemented(None),
        };

        let _ = subscriber.reject(error);
    }

    fn unsubscribe(&self, id: SubscriptionId) -> Result<bool> {
        let res = self.heads_subscribers.write().remove(&id).is_some();
        let res2 = self.logs_subscribers.write().remove(&id).is_some();

        Ok(res || res2)
    }
}
