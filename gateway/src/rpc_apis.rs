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

use std::cmp::PartialEq;
use std::collections::{BTreeMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;

use client::Client;
use futures_cpupool::CpuPool;
use jsonrpc_core::{self as core, MetaIoHandler};
use parity_reactor;
use parity_rpc::informant::ActivityNotifier;
use parity_rpc::{Host, Metadata};

use impls::{EthClient, EthFilterClient, EthSigningClient, NetClient, TracesClient, Web3Client};

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum Api {
    /// Web3 (Safe)
    Web3,
    /// Net (Safe)
    Net,
    /// Eth (Safe)
    Eth,
    /// Eth Pub-Sub (Safe)
    EthPubSub,
    /// Traces (Safe)
    Traces,
}

impl FromStr for Api {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use self::Api::*;

        match s {
            "web3" => Ok(Web3),
            "net" => Ok(Net),
            "eth" => Ok(Eth),
            "pubsub" => Ok(EthPubSub),
            "traces" => Ok(Traces),
            api => Err(format!("Unknown api: {}", api)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ApiSet {
    // Safe context (like token-protected WS interface)
    SafeContext,
    // Unsafe context (like jsonrpc over http)
    UnsafeContext,
    // All possible APIs
    All,
    // Local "unsafe" context and accounts access
    IpcContext,
    // APIs for Parity Generic Pub-Sub
    PubSub,
    // Fixed list of APis
    List(HashSet<Api>),
}

impl Default for ApiSet {
    fn default() -> Self {
        ApiSet::UnsafeContext
    }
}

impl PartialEq for ApiSet {
    fn eq(&self, other: &Self) -> bool {
        self.list_apis() == other.list_apis()
    }
}

impl FromStr for ApiSet {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut apis = HashSet::new();

        for api in s.split(',') {
            match api {
                "all" => {
                    apis.extend(ApiSet::All.list_apis());
                }
                "safe" => {
                    // Safe APIs are those that are safe even in UnsafeContext.
                    apis.extend(ApiSet::UnsafeContext.list_apis());
                }
                // Remove the API
                api if api.starts_with("-") => {
                    let api = api[1..].parse()?;
                    apis.remove(&api);
                }
                api => {
                    let api = api.parse()?;
                    apis.insert(api);
                }
            }
        }

        Ok(ApiSet::List(apis))
    }
}

/// Client Notifier
pub struct ClientNotifier {
    /// Client
    pub client: Arc<Client>,
}

impl ActivityNotifier for ClientNotifier {
    fn active(&self) {
        // TODO: anything needed to keep client alive?
        //self.client.keep_alive()
    }
}

/// RPC dependencies can be used to initialize RPC endpoints from APIs.
pub trait Dependencies {
    type Notifier: ActivityNotifier;

    /// Create the activity notifier.
    fn activity_notifier(&self) -> Self::Notifier;

    /// Extend the given I/O handler with endpoints for each API.
    fn extend_with_set<S>(&self, handler: &mut MetaIoHandler<Metadata, S>, apis: &HashSet<Api>)
    where
        S: core::Middleware<Metadata>;
}

/// RPC dependencies for a full node.
pub struct FullDependencies {
    pub client: Arc<Client>,
    pub ws_address: Option<Host>,
    pub pool: CpuPool,
    pub remote: parity_reactor::Remote,
}

impl FullDependencies {
    fn extend_api<S>(
        &self,
        handler: &mut MetaIoHandler<Metadata, S>,
        apis: &HashSet<Api>,
        for_generic_pubsub: bool,
    ) where
        S: core::Middleware<Metadata>,
    {
        use parity_rpc::v1::{Eth, EthFilter, EthSigning, Net, Traces, Web3};

        for api in apis {
            match *api {
                Api::Web3 => {
                    handler.extend_with(Web3Client::new().to_delegate());
                }
                Api::Net => {
                    handler.extend_with(NetClient::new().to_delegate());
                }
                Api::Eth => {
                    let client = EthClient::new(&self.client);
                    handler.extend_with(client.to_delegate());

                    if !for_generic_pubsub {
                        let filter_client = EthFilterClient::new(self.client.clone());
                        handler.extend_with(filter_client.to_delegate());

                        let signing_client = EthSigningClient::new(self.client.clone());
                        handler.extend_with(signing_client.to_delegate());
                    }
                }
                Api::EthPubSub => {
                    // TODO: pub/sub
                }
                Api::Traces => handler.extend_with(TracesClient::new().to_delegate()),
            }
        }
    }
}

impl Dependencies for FullDependencies {
    type Notifier = ClientNotifier;

    fn activity_notifier(&self) -> ClientNotifier {
        ClientNotifier {
            client: self.client.clone(),
        }
    }

    fn extend_with_set<S>(&self, handler: &mut MetaIoHandler<Metadata, S>, apis: &HashSet<Api>)
    where
        S: core::Middleware<Metadata>,
    {
        self.extend_api(handler, apis, false)
    }
}

impl ApiSet {
    pub fn list_apis(&self) -> HashSet<Api> {
        let mut public_list: HashSet<Api> = [Api::Web3, Api::Net, Api::Eth, Api::EthPubSub]
            .into_iter()
            .cloned()
            .collect();

        match *self {
            ApiSet::List(ref apis) => apis.clone(),
            ApiSet::UnsafeContext => {
                public_list.insert(Api::Traces);
                public_list
            }
            ApiSet::IpcContext => {
                public_list.insert(Api::Traces);
                public_list
            }
            ApiSet::SafeContext => {
                public_list.insert(Api::Traces);
                public_list
            }
            ApiSet::All => {
                public_list.insert(Api::Traces);
                public_list
            }
            ApiSet::PubSub => [Api::Eth, Api::Traces].into_iter().cloned().collect(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::{Api, ApiSet};

    #[test]
    fn test_api_parsing() {
        assert_eq!(Api::Web3, "web3".parse().unwrap());
        assert_eq!(Api::Net, "net".parse().unwrap());
        assert_eq!(Api::Eth, "eth".parse().unwrap());
        assert_eq!(Api::EthPubSub, "pubsub".parse().unwrap());
        assert_eq!(Api::Traces, "traces".parse().unwrap());
        assert!("rp".parse::<Api>().is_err());
    }

    #[test]
    fn test_api_set_default() {
        assert_eq!(ApiSet::UnsafeContext, ApiSet::default());
    }

    #[test]
    fn test_api_set_parsing() {
        assert_eq!(
            ApiSet::List(vec![Api::Web3, Api::Eth].into_iter().collect()),
            "web3,eth".parse().unwrap()
        );
    }

    #[test]
    fn test_api_set_unsafe_context() {
        let expected = vec![
            // make sure this list contains only SAFE methods
            Api::Web3,
            Api::Net,
            Api::Eth,
            Api::EthPubSub,
            Api::Traces,
        ].into_iter()
            .collect();
        assert_eq!(ApiSet::UnsafeContext.list_apis(), expected);
    }

    #[test]
    fn test_api_set_ipc_context() {
        let expected = vec![
            // safe
            Api::Web3,
            Api::Net,
            Api::Eth,
            Api::EthPubSub,
            Api::Traces,
        ].into_iter()
            .collect();
        assert_eq!(ApiSet::IpcContext.list_apis(), expected);
    }

    #[test]
    fn test_api_set_safe_context() {
        let expected = vec![
            // safe
            Api::Web3,
            Api::Net,
            Api::Eth,
            Api::EthPubSub,
            Api::Traces,
        ].into_iter()
            .collect();
        assert_eq!(ApiSet::SafeContext.list_apis(), expected);
    }

    #[test]
    fn test_all_apis() {
        assert_eq!(
            "all".parse::<ApiSet>().unwrap(),
            ApiSet::List(
                vec![Api::Web3, Api::Net, Api::Eth, Api::EthPubSub, Api::Traces]
                    .into_iter()
                    .collect()
            )
        );
    }

    #[test]
    fn test_safe_parsing() {
        assert_eq!(
            "safe".parse::<ApiSet>().unwrap(),
            ApiSet::List(
                vec![Api::Web3, Api::Net, Api::Eth, Api::EthPubSub, Api::Traces]
                    .into_iter()
                    .collect()
            )
        );
    }
}
