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

use ethcore::{filter::Filter as EthcoreFilter, ids::BlockId};
use ethereum_types::{Address, H256, H64, U256};
use failure::Error;
use jsonrpc_core::{
    futures::{future, Future},
    BoxFuture, Result,
};
use jsonrpc_macros::Trailing;
use lazy_static::lazy_static;
use oasis_core_runtime::common::logger::get_logger;
use parity_rpc::v1::{
    helpers::{errors, fake_sign},
    metadata::Metadata,
    traits::Eth,
    types::{
        BlockNumber, Bytes, CallRequest, Filter, Index, Log as RpcLog, Receipt as RpcReceipt,
        RichBlock, Transaction as RpcTransaction, Work, H160 as RpcH160, H256 as RpcH256,
        H64 as RpcH64, U256 as RpcU256,
    },
};
use prometheus::{
    __register_counter_vec, histogram_opts, labels, opts, register_histogram_vec,
    register_int_counter_vec, HistogramVec, IntCounterVec,
};
use slog::{debug, info, Logger};

use crate::{
    translator::Translator,
    util::{block_number_to_id, execution_error, jsonrpc_error},
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
    translator: Arc<Translator>,
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

impl EthClient {
    /// Creates new EthClient.
    pub fn new(translator: Arc<Translator>) -> Self {
        EthClient {
            logger: get_logger("gateway/impls/eth"),
            translator,
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
        Ok(self.translator.gas_price().into())
    }

    fn accounts(&self, _meta: Metadata) -> Result<Vec<RpcH160>> {
        ETH_RPC_CALLS.with(&labels! {"call" => "accounts",}).inc();
        Ok(vec![])
    }

    fn block_number(&self) -> BoxFuture<RpcU256> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "blockNumber",})
            .inc();

        Box::new(
            self.translator
                .get_latest_block()
                .map(|blk| RpcU256::from(blk.number()))
                .map_err(jsonrpc_error),
        )
    }

    fn balance(&self, address: RpcH160, num: Trailing<BlockNumber>) -> BoxFuture<RpcU256> {
        ETH_RPC_CALLS.with(&labels! {"call" => "getBalance",}).inc();

        let address = address.into();
        let num = num.unwrap_or_default();

        info!(self.logger, "eth_getBalance"; "address" => ?address, "num" => ?num);

        Box::new(
            self.translator
                .get_block_unwrap(block_number_to_id(num))
                .and_then(move |blk| Ok(blk.state()?.balance(&address)?.into()))
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

        let address = address.into();
        let pos: U256 = RpcU256::into(pos);
        let num = num.unwrap_or_default();

        info!(
            self.logger,
            "eth_getStorageAt";
                "address" => ?address,
                "position" => ?pos,
                "num" => ?num
        );

        let pos = pos.into();

        Box::new(
            self.translator
                .get_block_unwrap(block_number_to_id(num))
                .and_then(move |blk| Ok(blk.state()?.storage_at(&address, &pos)?.into()))
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

        Box::new(
            self.translator
                .get_block_unwrap(block_number_to_id(num))
                .and_then(move |blk| Ok(blk.state()?.nonce(&address)?.into()))
                .map_err(jsonrpc_error),
        )
    }

    fn block_transaction_count_by_hash(&self, hash: RpcH256) -> BoxFuture<Option<RpcU256>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getBlockTransactionCountByHash",})
            .inc();
        info!(self.logger, "eth_getBlockTransactionCountByHash"; "hash" => ?hash);

        Box::new(
            self.translator
                .get_block_by_hash(hash.into())
                .and_then(|blk| -> Box<dyn Future<Item = _, Error = Error> + Send> {
                    match blk {
                        Some(blk) => {
                            Box::new(blk.raw_transactions().map(|txns| Some(txns.count().into())))
                        }
                        None => Box::new(future::ok(None)),
                    }
                })
                .map_err(jsonrpc_error),
        )
    }

    fn block_transaction_count_by_number(&self, num: BlockNumber) -> BoxFuture<Option<RpcU256>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getBlockTransactionCountByNumber",})
            .inc();
        info!(self.logger, "eth_getBlockTransactionCountByNumber"; "num" => ?num);

        // We don't have pending transactions.
        if let BlockNumber::Pending = num {
            return Box::new(future::ok(Some(0.into())));
        }

        Box::new(
            self.translator
                .get_block(block_number_to_id(num))
                .and_then(|blk| -> Box<dyn Future<Item = _, Error = Error> + Send> {
                    match blk {
                        Some(blk) => {
                            Box::new(blk.raw_transactions().map(|txns| Some(txns.count().into())))
                        }
                        None => Box::new(future::ok(None)),
                    }
                })
                .map_err(jsonrpc_error),
        )
    }

    fn block_uncles_count_by_hash(&self, _hash: RpcH256) -> BoxFuture<Option<RpcU256>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getUncleCountByBlockHash",})
            .inc();

        // We do not have uncles.
        Box::new(future::ok(None))
    }

    fn block_uncles_count_by_number(&self, _num: BlockNumber) -> BoxFuture<Option<RpcU256>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getUncleCountByBlockNumber",})
            .inc();

        // We do not have uncles.
        Box::new(future::ok(None))
    }

    fn code_at(&self, address: RpcH160, num: Trailing<BlockNumber>) -> BoxFuture<Bytes> {
        ETH_RPC_CALLS.with(&labels! {"call" => "getCode",}).inc();

        let address: Address = RpcH160::into(address);
        let num = num.unwrap_or_default();

        info!(self.logger, "eth_getCode"; "address" => ?address, "num" => ?num);

        Box::new(
            self.translator
                .get_block_unwrap(block_number_to_id(num))
                .and_then(move |blk| {
                    Ok(blk
                        .state()?
                        .code(&address)?
                        .map_or_else(Bytes::default, |b| Bytes::new((&*b).clone())))
                })
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

        Box::new(
            self.translator
                .get_block_by_hash(hash.into())
                .and_then(
                    move |blk| -> Box<dyn Future<Item = _, Error = Error> + Send> {
                        match blk {
                            Some(blk) => Box::new(blk.rich_block(include_txs).map(Some)),
                            None => Box::new(future::ok(None)),
                        }
                    },
                )
                .map_err(jsonrpc_error),
        )
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

        Box::new(
            self.translator
                .get_block(block_number_to_id(num))
                .and_then(
                    move |blk| -> Box<dyn Future<Item = _, Error = Error> + Send> {
                        match blk {
                            Some(blk) => Box::new(blk.rich_block(include_txs).map(Some)),
                            None => Box::new(future::ok(None)),
                        }
                    },
                )
                .map_err(jsonrpc_error),
        )
    }

    fn transaction_by_hash(&self, hash: RpcH256) -> BoxFuture<Option<RpcTransaction>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getTransactionByHash",})
            .inc();
        info!(self.logger, "eth_getTransactionByHash"; "hash" => ?hash);

        let hash = hash.into();

        Box::new(
            self.translator
                .get_txn_by_hash(hash)
                .and_then(move |txn| {
                    txn.map(|txn| Ok(RpcTransaction::from_localized(txn.transaction()?)))
                        .transpose()
                })
                .map_err(jsonrpc_error),
        )
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

        let hash = hash.into();

        Box::new(
            self.translator
                .get_txn_by_block_hash_and_index(hash, index.value() as u32)
                .and_then(move |txn| {
                    txn.map(|txn| Ok(RpcTransaction::from_localized(txn.transaction()?)))
                        .transpose()
                })
                .map_err(jsonrpc_error),
        )
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

        // We don't have pending transactions.
        if let BlockNumber::Pending = num {
            return Box::new(future::ok(None));
        }

        Box::new(
            self.translator
                .get_txn(block_number_to_id(num), index.value() as u32)
                .and_then(move |txn| {
                    txn.map(|txn| Ok(RpcTransaction::from_localized(txn.transaction()?)))
                        .transpose()
                })
                .map_err(jsonrpc_error),
        )
    }

    fn transaction_receipt(&self, hash: RpcH256) -> BoxFuture<Option<RpcReceipt>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getTransactionReceipt",})
            .inc();

        let hash: H256 = hash.into();
        info!(self.logger, "eth_getTransactionReceipt"; "hash" => ?hash);

        Box::new(
            self.translator
                .get_txn_by_hash(hash)
                .and_then(|txn| txn.map(|txn| Ok(txn.receipt()?.into())).transpose())
                .map_err(jsonrpc_error),
        )
    }

    fn uncle_by_block_hash_and_index(
        &self,
        _hash: RpcH256,
        _index: Index,
    ) -> BoxFuture<Option<RichBlock>> {
        ETH_RPC_CALLS
            .with(&labels! {"call" => "getUncleByBlockHashAndIndex",})
            .inc();

        // We do not have uncles.
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

        // We do not have uncles.
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

        Box::new(
            self.translator
                .clone()
                .logs(filter)
                .map_err(jsonrpc_error)
                .map(|logs| logs.into_iter().map(Into::into).collect()),
        )
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
            self.translator
                .send_raw_transaction(raw.into())
                .map(|(hash, _result)| hash.into())
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

        let signed = try_bf!(fake_sign::sign_call(request.into(), meta.is_dapp()));

        Box::new(
            self.translator
                .simulate_transaction(signed, block_number_to_id(num))
                .map_err(errors::call)
                .and_then(|executed| match executed.exception {
                    Some(ref exception) => Err(errors::vm(exception, &executed.output)),
                    None => Ok(executed),
                })
                .map(|executed| executed.output.into())
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

        let signed = try_bf!(fake_sign::sign_call(request.into(), meta.is_dapp()));

        Box::new(
            self.translator
                .estimate_gas(signed, block_number_to_id(num))
                .map_err(execution_error)
                .map(Into::into)
                .then(move |result| {
                    drop(timer);
                    result
                }),
        )
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
