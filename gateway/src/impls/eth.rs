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

//! Eth rpc implementation.

use std::{collections::BTreeMap, sync::Arc};

use ekiden_runtime::common::logger::get_logger;
use ethcore::{
    filter::Filter as EthcoreFilter,
    ids::{BlockId, TransactionId},
};
use ethereum_types::{Address, H256, H64, U256};
use failure::format_err;
use jsonrpc_core::{
    futures::{future, Future},
    BoxFuture, Result,
};
use jsonrpc_macros::Trailing;
use lazy_static::lazy_static;
use parity_rpc::v1::{
    helpers::{errors, fake_sign, limit_logs},
    metadata::Metadata,
    traits::Eth,
    types::{
        block_number_to_id, Block, BlockNumber, BlockTransactions, Bytes, CallRequest, Filter,
        Index, Log as RpcLog, Receipt as RpcReceipt, RichBlock, Transaction as RpcTransaction,
        Work, H160 as RpcH160, H256 as RpcH256, H64 as RpcH64, U256 as RpcU256,
    },
};
use prometheus::{
    __register_counter_vec, histogram_opts, labels, opts, register_histogram_vec,
    register_int_counter_vec, HistogramVec, IntCounterVec,
};
use slog::{debug, info, Logger};

use crate::{
    client::Client,
    util::{execution_error, jsonrpc_error},
};

// Metrics.
lazy_static! {
    static ref ETH_RPC_CALLS: IntCounterVec = register_int_counter_vec!(
        "web3_gateway_eth_rpc_calls",
        "Number of eth API RPC calls",
        &["call"]
    )
    .unwrap();
    static ref ETH_RPC_CALL_TIME: HistogramVec = register_histogram_vec!(
        "web3_gateway_eth_rpc_call_time",
        "Time taken by eth API RPC calls",
        &["call"],
        vec![0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 25.0, 50.0]
    )
    .unwrap();
}

// short for "try_boxfuture"
// unwrap a result, returning a BoxFuture<_, Err> on failure.
macro_rules! try_bf {
    ($res:expr) => {
        match $res {
            Ok(val) => val,
            Err(e) => return Box::new(::jsonrpc_core::futures::future::err(e.into())),
        }
    };
}

lazy_static! {
    // dummy-valued PoW-related block extras
    static ref BLOCK_EXTRA_INFO: BTreeMap<String, String> = {
        let mut map = BTreeMap::new();
        map.insert("mixHash".into(), format!("0x{:x}", H256::default()));
        map.insert("nonce".into(), format!("0x{:x}", H64::default()));
        map
    };
}

/// Eth rpc implementation.
pub struct EthClient {
    logger: Logger,
    client: Arc<Client>,
    eip86_transition: u64,
}

#[derive(Debug)]
enum BlockNumberOrId {
    Number(BlockNumber),
    Id(BlockId),
}

impl From<BlockId> for BlockNumberOrId {
    fn from(value: BlockId) -> BlockNumberOrId {
        BlockNumberOrId::Id(value)
    }
}

impl From<BlockNumber> for BlockNumberOrId {
    fn from(value: BlockNumber) -> BlockNumberOrId {
        BlockNumberOrId::Number(value)
    }
}

enum PendingOrBlock {
    Block(BlockId),
    Pending,
}

enum PendingTransactionId {
    Hash(H256),
    Location(PendingOrBlock, usize),
}

impl EthClient {
    /// Creates new EthClient.
    pub fn new(client: &Arc<Client>) -> Self {
        EthClient {
            logger: get_logger("gateway/impls/eth"),
            client: client.clone(),
            eip86_transition: client.eip86_transition(),
        }
    }

    fn rich_block(&self, id: BlockNumberOrId, include_txs: bool) -> BoxFuture<Option<RichBlock>> {
        let block = match id {
            BlockNumberOrId::Number(num) => {
                // for "pending", just use latest block
                let id = match num {
                    BlockNumber::Latest => BlockId::Latest,
                    BlockNumber::Earliest => BlockId::Earliest,
                    BlockNumber::Num(n) => BlockId::Number(n),
                    BlockNumber::Pending => BlockId::Latest,
                };

                self.client.block(id)
            }

            BlockNumberOrId::Id(id) => self.client.block(id),
        };

        let eip86_transition = self.eip86_transition;

        Box::new(block.map_err(jsonrpc_error).map(move |block| match block {
            Some(block) => {
                let view = block.header_view();
                Some(RichBlock {
                    inner: Block {
                        hash: Some(view.hash().into()),
                        size: Some(block.rlp().as_raw().len().into()),
                        parent_hash: view.parent_hash().into(),
                        uncles_hash: view.uncles_hash().into(),
                        author: view.author().into(),
                        miner: view.author().into(),
                        state_root: view.state_root().into(),
                        transactions_root: view.transactions_root().into(),
                        receipts_root: view.receipts_root().into(),
                        number: Some(view.number().into()),
                        gas_used: view.gas_used().into(),
                        gas_limit: view.gas_limit().into(),
                        logs_bloom: Some(view.log_bloom().into()),
                        timestamp: view.timestamp().into(),
                        difficulty: view.difficulty().into(),
                        total_difficulty: Some(RpcU256::from(0)),
                        seal_fields: view.seal().into_iter().map(Into::into).collect(),
                        uncles: block.uncle_hashes().into_iter().map(Into::into).collect(),
                        transactions: match include_txs {
                            true => BlockTransactions::Full(
                                block
                                    .view()
                                    .localized_transactions()
                                    .into_iter()
                                    .map(|t| RpcTransaction::from_localized(t, eip86_transition))
                                    .collect(),
                            ),
                            false => BlockTransactions::Hashes(
                                block
                                    .transaction_hashes()
                                    .into_iter()
                                    .map(Into::into)
                                    .collect(),
                            ),
                        },
                        extra_data: Bytes::new(view.extra_data()),
                    },
                    extra_info: BLOCK_EXTRA_INFO.clone(),
                })
            }
            _ => None,
        }))
    }

    fn transaction(&self, id: PendingTransactionId) -> Result<Option<RpcTransaction>> {
        let client_transaction = |id| match self.client.transaction(id) {
            Some(t) => Ok(Some(RpcTransaction::from_localized(
                t,
                self.eip86_transition,
            ))),
            None => Ok(None),
        };

        match id {
            PendingTransactionId::Hash(hash) => client_transaction(TransactionId::Hash(hash)),
            PendingTransactionId::Location(PendingOrBlock::Block(block), index) => {
                client_transaction(TransactionId::Location(block, index))
            }
            PendingTransactionId::Location(PendingOrBlock::Pending, _index) => {
                // we don't have pending blocks
                Ok(None)
            }
        }
    }

    pub fn get_block_id(number: BlockNumber) -> BlockId {
        // for "pending", just use latest block
        match number {
            BlockNumber::Num(num) => BlockId::Number(num),
            BlockNumber::Earliest => BlockId::Earliest,
            BlockNumber::Latest => BlockId::Latest,
            BlockNumber::Pending => BlockId::Latest,
        }
    }
}

impl Eth for EthClient {
    type Metadata = Metadata;

    fn protocol_version(&self) -> Result<String> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "protocolVersion",})
            .inc();
        // Ethereum wire protocol version: https://github.com/ethereum/wiki/wiki/Ethereum-Wire-Protocol#fast-synchronization-pv63
        Ok(format!("{}", 63))
    }

    fn syncing(&self) -> Result<bool> {
        ETH_RPC_CALLS.with(&labels! {"call" => "syncing",}).inc();
        Ok(false)
    }

    fn author(&self, _meta: Metadata) -> Result<RpcH160> {
        ETH_RPC_CALLS.with(&labels! {"call" => "coinbase",}).inc();
        Ok(Default::default())
    }

    fn is_mining(&self) -> Result<bool> {
        ETH_RPC_CALLS.with(&labels! {"call" => "mining",}).inc();
        Ok(true)
    }

    fn hashrate(&self) -> Result<RpcU256> {
        ETH_RPC_CALLS.with(&labels! {"call" => "hashrate",}).inc();
        Ok(RpcU256::from(0))
    }

    fn gas_price(&self) -> Result<RpcU256> {
        ETH_RPC_CALLS.with(&labels! {"call" => "gasPrice",}).inc();
        Ok(RpcU256::from(self.client.gas_price()))
    }

    fn accounts(&self, _meta: Metadata) -> Result<Vec<RpcH160>> {
        ETH_RPC_CALLS.with(&labels! {"call" => "accounts",}).inc();
        Ok(vec![])
    }

    fn block_number(&self) -> Result<RpcU256> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "blockNumber",})
            .inc();
        Ok(RpcU256::from(self.client.best_block_number()))
    }

    fn balance(&self, address: RpcH160, num: Trailing<BlockNumber>) -> BoxFuture<RpcU256> {
        ETH_RPC_CALLS.with(&labels! {"call" => "getBalance",}).inc();
        let address = address.into();
        let num = num.unwrap_or_default();

        info!(self.logger, "eth_getBalance"; "address" => ?address, "num" => ?num);

        Box::new(
            self.client
                .balance(&address, Self::get_block_id(num))
                .map(|balance| balance.into())
                .map_err(jsonrpc_error),
        )
    }

    fn storage_at(
        &self,
        address: RpcH160,
        pos: RpcU256,
        num: Trailing<BlockNumber>,
    ) -> BoxFuture<RpcH256> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getStorageAt",})
            .inc();
        let address: Address = RpcH160::into(address);
        let position: U256 = RpcU256::into(pos);
        let num = num.unwrap_or_default();

        info!(
            self.logger,
            "eth_getStorageAt";
                "address" => ?address,
                "position" => ?position,
                "num" => ?num
        );

        Box::new(
            self.client
                .storage_at(&address, &H256::from(position), Self::get_block_id(num))
                .map(|hash| hash.into())
                .map_err(jsonrpc_error),
        )
    }

    fn transaction_count(
        &self,
        address: RpcH160,
        num: Trailing<BlockNumber>,
    ) -> BoxFuture<RpcU256> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getTransactionCount",})
            .inc();
        let address: Address = RpcH160::into(address);
        let num = num.unwrap_or_default();

        info!(
            self.logger,
            "eth_getTransactionCount";
                "address" => ?address,
                "num" => ?num
        );

        let result: BoxFuture<U256> = match num {
            BlockNumber::Pending => Box::new(
                self.client
                    .nonce(&address, BlockId::Latest)
                    .map_err(|_error| errors::database("latest nonce missing")),
            ),
            number => Box::new(
                self.client
                    .nonce(&address, block_number_to_id(number))
                    .map_err(jsonrpc_error),
            ),
        };

        Box::new(result.map(|nonce| nonce.into()))
    }

    fn block_transaction_count_by_hash(&self, hash: RpcH256) -> BoxFuture<Option<RpcU256>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getBlockTransactionCountByHash",})
            .inc();
        info!(self.logger, "eth_getBlockTransactionCountByHash"; "hash" => ?hash);

        Box::new(
            self.client
                .block(BlockId::Hash(hash.into()))
                .map_err(jsonrpc_error)
                .map(|block| block.map(|block| block.transactions_count().into())),
        )
    }

    fn block_transaction_count_by_number(&self, num: BlockNumber) -> BoxFuture<Option<RpcU256>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getBlockTransactionCountByNumber",})
            .inc();
        info!(self.logger, "eth_getBlockTransactionCountByNumber"; "num" => ?num);

        match num {
            // we don't have pending blocks
            BlockNumber::Pending => Box::new(future::ok(Some(RpcU256::from(0)))),
            _ => Box::new(
                self.client
                    .block(block_number_to_id(num))
                    .map_err(jsonrpc_error)
                    .map(|block| block.map(|block| block.transactions_count().into())),
            ),
        }
    }

    fn block_uncles_count_by_hash(&self, hash: RpcH256) -> BoxFuture<Option<RpcU256>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getUncleCountByBlockHash",})
            .inc();

        Box::new(
            self.client
                .block(BlockId::Hash(hash.into()))
                .map_err(jsonrpc_error)
                .map(|block| block.map(|block| block.uncles_count().into())),
        )
    }

    fn block_uncles_count_by_number(&self, num: BlockNumber) -> BoxFuture<Option<RpcU256>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getUncleCountByBlockNumber",})
            .inc();

        match num {
            BlockNumber::Pending => Box::new(future::ok(Some(0.into()))),
            _ => Box::new(
                self.client
                    .block(block_number_to_id(num))
                    .map_err(jsonrpc_error)
                    .map(|block| block.map(|block| block.uncles_count().into())),
            ),
        }
    }

    fn code_at(&self, address: RpcH160, num: Trailing<BlockNumber>) -> BoxFuture<Bytes> {
        ETH_RPC_CALLS.with(&labels! {"call" => "getCode",}).inc();
        let address: Address = RpcH160::into(address);
        let num = num.unwrap_or_default();

        info!(self.logger, "eth_getCode"; "address" => ?address, "num" => ?num);

        Box::new(
            self.client
                .code(&address, Self::get_block_id(num))
                .map(|code| code.map_or_else(Bytes::default, Bytes::new))
                .map_err(jsonrpc_error),
        )
    }

    fn block_by_hash(&self, hash: RpcH256, include_txs: bool) -> BoxFuture<Option<RichBlock>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getBlockByHash",})
            .inc();
        info!(
            self.logger,
            "eth_getBlockByHash";
                "hash" => ?hash,
                "include_txs" => ?include_txs
        );

        self.rich_block(BlockId::Hash(hash.into()).into(), include_txs)
    }

    fn block_by_number(&self, num: BlockNumber, include_txs: bool) -> BoxFuture<Option<RichBlock>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getBlockByNumber",})
            .inc();
        info!(
            self.logger,
            "eth_getBlockByNumber";
                "num" => ?num,
                "include_txs" => ?include_txs
        );

        self.rich_block(num.into(), include_txs)
    }

    fn transaction_by_hash(&self, hash: RpcH256) -> BoxFuture<Option<RpcTransaction>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getTransactionByHash",})
            .inc();
        info!(self.logger, "eth_getTransactionByHash"; "hash" => ?hash);

        let hash: H256 = hash.into();
        let tx = try_bf!(self.transaction(PendingTransactionId::Hash(hash)));
        Box::new(future::ok(tx))
    }

    fn transaction_by_block_hash_and_index(
        &self,
        hash: RpcH256,
        index: Index,
    ) -> BoxFuture<Option<RpcTransaction>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getTransactionByBlockHashAndIndex",})
            .inc();
        info!(
            self.logger,
            "eth_getTransactionByBlockHashAndIndex";
                "hash" => ?hash,
                "index" => ?index
        );

        let id = PendingTransactionId::Location(
            PendingOrBlock::Block(BlockId::Hash(hash.into())),
            index.value(),
        );
        Box::new(future::done(self.transaction(id)))
    }

    fn transaction_by_block_number_and_index(
        &self,
        num: BlockNumber,
        index: Index,
    ) -> BoxFuture<Option<RpcTransaction>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getTransactionByBlockNumberAndIndex",})
            .inc();
        info!(
            self.logger,
            "eth_getTransactionByBlockNumberAndIndex";
                "num" => ?num,
                "index" => ?index
        );

        let block_id = match num {
            BlockNumber::Latest => PendingOrBlock::Block(BlockId::Latest),
            BlockNumber::Earliest => PendingOrBlock::Block(BlockId::Earliest),
            BlockNumber::Num(num) => PendingOrBlock::Block(BlockId::Number(num)),
            BlockNumber::Pending => PendingOrBlock::Pending,
        };

        let transaction_id = PendingTransactionId::Location(block_id, index.value());
        Box::new(future::done(self.transaction(transaction_id)))
    }

    fn transaction_receipt(&self, hash: RpcH256) -> BoxFuture<Option<RpcReceipt>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getTransactionReceipt",})
            .inc();
        let hash: H256 = hash.into();
        info!(self.logger, "eth_getTransactionReceipt"; "hash" => ?hash);

        let receipt = self.client.transaction_receipt(hash);
        Box::new(future::ok(receipt.map(Into::into)))
    }

    fn uncle_by_block_hash_and_index(
        &self,
        _hash: RpcH256,
        _index: Index,
    ) -> BoxFuture<Option<RichBlock>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getUncleByBlockHashAndIndex",})
            .inc();
        // we dont' have uncles
        Box::new(future::ok(None))
    }

    fn uncle_by_block_number_and_index(
        &self,
        _num: BlockNumber,
        _index: Index,
    ) -> BoxFuture<Option<RichBlock>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getUncleByBlockNumberAndIndex",})
            .inc();
        // we dont' have uncles
        Box::new(future::ok(None))
    }

    fn compilers(&self) -> Result<Vec<String>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getCompilers",})
            .inc();
        Err(errors::deprecated(
            "Compilation functionality is deprecated.".to_string(),
        ))
    }

    fn logs(&self, filter: Filter) -> BoxFuture<Vec<RpcLog>> {
        ETH_RPC_CALLS.with(&labels! {"call" => "getLogs",}).inc();
        info!(self.logger, "eth_getLogs"; "filter" => ?filter);

        let filter: EthcoreFilter = filter.into();

        // Temporary mitigation for #397: check filter block range
        if !self.client.check_filter_range(filter.clone()) {
            return Box::new(future::err(jsonrpc_error(format_err!(
                "Filter exceeds allowed block range"
            ))));
        }

        let logs = self
            .client
            .logs(filter.clone())
            .into_iter()
            .map(From::from)
            .collect::<Vec<RpcLog>>();
        let logs = limit_logs(logs, filter.limit);
        Box::new(future::ok(logs))
    }

    fn work(&self, _no_new_work_timeout: Trailing<u64>) -> Result<Work> {
        ETH_RPC_CALLS.with(&labels! {"call" => "getWork",}).inc();
        Err(errors::unimplemented(None))
    }

    fn submit_work(&self, _nonce: RpcH64, _pow_hash: RpcH256, _mix_hash: RpcH256) -> Result<bool> {
        ETH_RPC_CALLS.with(&labels! {"call" => "submitWork",}).inc();
        Err(errors::unimplemented(None))
    }

    fn submit_hashrate(&self, _rate: RpcU256, _id: RpcH256) -> Result<bool> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "submitHashrate",})
            .inc();
        Err(errors::unimplemented(None))
    }

    fn send_raw_transaction(&self, raw: Bytes) -> BoxFuture<RpcH256> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "sendRawTransaction",})
            .inc();
        let timer = ETH_RPC_CALL_TIME
            .with(&labels! {"call" => "sendRawTransaction",})
            .start_timer();

        if log_enabled!(log::LogLevel::Debug) {
            debug!(self.logger, "eth_sendRawTransaction"; "data" => ?raw);
        } else {
            info!(self.logger, "eth_sendRawTransaction")
        }

        Box::new(
            self.client
                .send_raw_transaction(raw.into())
                .map(Into::into)
                .map_err(execution_error)
                .then(move |result| {
                    drop(timer);
                    result
                }),
        )
    }

    fn submit_transaction(&self, raw: Bytes) -> BoxFuture<RpcH256> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "submitTransaction",})
            .inc();
        info!(self.logger, "eth_submitTransaction"; "data" => ?raw);

        self.send_raw_transaction(raw)
    }

    fn call(
        &self,
        meta: Self::Metadata,
        request: CallRequest,
        num: Trailing<BlockNumber>,
    ) -> BoxFuture<Bytes> {
        ETH_RPC_CALLS.with(&labels! {"call" => "call",}).inc();
        let timer = ETH_RPC_CALL_TIME
            .with(&labels! {"call" => "call",})
            .start_timer();
        let num = num.unwrap_or_default();

        info!(self.logger, "eth_call"; "request" => ?request, "num" => ?num);

        let request = CallRequest::into(request);
        let signed = try_bf!(fake_sign::sign_call(request, meta.is_dapp()));

        let client = self.client.clone();

        Box::new(
            future::lazy(move || {
                client
                    .call(&signed, Self::get_block_id(num))
                    .map_err(errors::call)
                    .and_then(|executed| match executed.exception {
                        Some(ref exception) => Err(errors::vm(exception, &executed.output)),
                        None => Ok(executed),
                    })
                    .map(|b| b.output.into())
            })
            .then(move |result| {
                drop(timer);
                result
            }),
        )
    }

    fn estimate_gas(
        &self,
        meta: Self::Metadata,
        request: CallRequest,
        num: Trailing<BlockNumber>,
    ) -> BoxFuture<RpcU256> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "estimateGas",})
            .inc();
        let timer = ETH_RPC_CALL_TIME
            .with(&labels! {"call" => "estimateGas",})
            .start_timer();
        let num = num.unwrap_or_default();

        info!(self.logger, "eth_estimateGas"; "request" => ?request, "num" => ?num);

        let request = CallRequest::into(request);
        let signed = try_bf!(fake_sign::sign_call(request, meta.is_dapp()));

        let is_confidential = match self.client.is_confidential(&signed) {
            Ok(conf) => conf,
            Err(e) => return Box::new(future::err(execution_error(e))),
        };

        if is_confidential {
            Box::new(
                self.client
                    .confidential_estimate_gas(&signed)
                    .map(Into::into)
                    .map_err(execution_error)
                    .then(move |result| {
                        drop(timer);
                        result
                    }),
            )
        } else {
            let client = self.client.clone();
            Box::new(
                future::lazy(move || {
                    client
                        .estimate_gas(&signed, Self::get_block_id(num))
                        .map_err(execution_error)
                        .map(Into::into)
                })
                .then(move |result| {
                    drop(timer);
                    result
                }),
            )
        }
    }

    fn compile_lll(&self, _: String) -> Result<Bytes> {
        ETH_RPC_CALLS.with(&labels! {"call" => "compileLLL",}).inc();
        Err(errors::deprecated(
            "Compilation of LLL via RPC is deprecated".to_string(),
        ))
    }

    fn compile_serpent(&self, _: String) -> Result<Bytes> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "compileSerpent",})
            .inc();
        Err(errors::deprecated(
            "Compilation of Serpent via RPC is deprecated".to_string(),
        ))
    }

    fn compile_solidity(&self, _: String) -> Result<Bytes> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "compileSolidity",})
            .inc();
        Err(errors::deprecated(
            "Compilation of Solidity via RPC is deprecated".to_string(),
        ))
    }
}
