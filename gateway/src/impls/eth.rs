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

use std::collections::BTreeMap;
use std::sync::Arc;

use ethereum_types::{Address, H256, H64, U256};

use client::Client;

use ethcore::filter::Filter as EthcoreFilter;
use ethcore::ids::{BlockId, TransactionId};

use jsonrpc_core::futures::{future, Future};
use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_macros::Trailing;

use log;

use parity_rpc::v1::helpers::{errors, fake_sign, limit_logs};
use parity_rpc::v1::metadata::Metadata;
use parity_rpc::v1::traits::Eth;
use parity_rpc::v1::types::{block_number_to_id, Block, BlockNumber, BlockTransactions, Bytes,
                            CallRequest, Filter, H160 as RpcH160, H256 as RpcH256, H64 as RpcH64,
                            Index, Log as RpcLog, Receipt as RpcReceipt, RichBlock,
                            Transaction as RpcTransaction, U256 as RpcU256, Work};

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

        Box::new(
            block
                .map_err(|error| errors::internal("runtime call failed", error))
                .map(move |block| match block {
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
                                            .map(|t| {
                                                RpcTransaction::from_localized(t, eip86_transition)
                                            })
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
                }),
        )
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
        measure_counter_inc!("protocolVersion");
        // Ethereum wire protocol version: https://github.com/ethereum/wiki/wiki/Ethereum-Wire-Protocol#fast-synchronization-pv63
        Ok(format!("{}", 63))
    }

    fn syncing(&self) -> Result<bool> {
        measure_counter_inc!("syncing");
        Ok(false)
    }

    fn author(&self, _meta: Metadata) -> Result<RpcH160> {
        measure_counter_inc!("coinbase");
        Ok(Default::default())
    }

    fn is_mining(&self) -> Result<bool> {
        measure_counter_inc!("mining");
        Ok(true)
    }

    fn hashrate(&self) -> Result<RpcU256> {
        measure_counter_inc!("hashrate");
        Ok(RpcU256::from(0))
    }

    fn gas_price(&self) -> Result<RpcU256> {
        measure_counter_inc!("gasPrice");
        Ok(RpcU256::from(self.client.gas_price()))
    }

    fn accounts(&self, _meta: Metadata) -> Result<Vec<RpcH160>> {
        measure_counter_inc!("accounts");
        Ok(vec![])
    }

    fn block_number(&self) -> Result<RpcU256> {
        measure_counter_inc!("blockNumber");
        Ok(RpcU256::from(self.client.best_block_number()))
    }

    fn balance(&self, address: RpcH160, num: Trailing<BlockNumber>) -> BoxFuture<RpcU256> {
        measure_counter_inc!("getBalance");
        let address = address.into();
        let num = num.unwrap_or_default();

        info!("eth_getBalance(address: {:?}, number: {:?})", address, num);

        Box::new(
            self.client
                .balance(&address, Self::get_block_id(num))
                .map(|balance| balance.into())
                .map_err(|_error| errors::state_pruned()),
        )
    }

    fn storage_at(
        &self,
        address: RpcH160,
        pos: RpcU256,
        num: Trailing<BlockNumber>,
    ) -> BoxFuture<RpcH256> {
        measure_counter_inc!("getStorageAt");
        let address: Address = RpcH160::into(address);
        let position: U256 = RpcU256::into(pos);
        let num = num.unwrap_or_default();

        info!(
            "eth_getStorageAt(address: {:?}, position: {:?}, number: {:?})",
            address, position, num
        );

        Box::new(
            self.client
                .storage_at(&address, &H256::from(position), Self::get_block_id(num))
                .map(|hash| hash.into())
                .map_err(|_error| errors::state_pruned()),
        )
    }

    fn transaction_count(
        &self,
        address: RpcH160,
        num: Trailing<BlockNumber>,
    ) -> BoxFuture<RpcU256> {
        measure_counter_inc!("getTransactionCount");
        let address: Address = RpcH160::into(address);
        let num = num.unwrap_or_default();

        info!(
            "eth_getTransactionCount(address: {:?}, number: {:?})",
            address, num
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
                    .map_err(|_error| errors::state_pruned()),
            ),
        };

        Box::new(result.map(|nonce| nonce.into()))
    }

    fn block_transaction_count_by_hash(&self, hash: RpcH256) -> BoxFuture<Option<RpcU256>> {
        measure_counter_inc!("getBlockTransactionCountByHash");
        info!("eth_getBlockTransactionCountByHash(hash: {:?})", hash);
        Box::new(
            self.client
                .block(BlockId::Hash(hash.into()))
                .map_err(|error| errors::internal("runtime call failed", error))
                .map(|block| block.map(|block| block.transactions_count().into())),
        )
    }

    fn block_transaction_count_by_number(&self, num: BlockNumber) -> BoxFuture<Option<RpcU256>> {
        measure_counter_inc!("getBlockTransactionCountByNumber");
        info!("eth_getBlockTransactionCountByNumber(number: {:?})", num);
        match num {
            // we don't have pending blocks
            BlockNumber::Pending => Box::new(future::ok(Some(RpcU256::from(0)))),
            _ => Box::new(
                self.client
                    .block(block_number_to_id(num))
                    .map_err(|error| errors::internal("runtime call failed", error))
                    .map(|block| block.map(|block| block.transactions_count().into())),
            ),
        }
    }

    fn block_uncles_count_by_hash(&self, hash: RpcH256) -> BoxFuture<Option<RpcU256>> {
        measure_counter_inc!("getUncleCountByBlockHash");
        Box::new(
            self.client
                .block(BlockId::Hash(hash.into()))
                .map_err(|error| errors::internal("runtime call failed", error))
                .map(|block| block.map(|block| block.uncles_count().into())),
        )
    }

    fn block_uncles_count_by_number(&self, num: BlockNumber) -> BoxFuture<Option<RpcU256>> {
        measure_counter_inc!("getUncleCountByBlockNumber");
        match num {
            BlockNumber::Pending => Box::new(future::ok(Some(0.into()))),
            _ => Box::new(
                self.client
                    .block(block_number_to_id(num))
                    .map_err(|error| errors::internal("runtime call failed", error))
                    .map(|block| block.map(|block| block.uncles_count().into())),
            ),
        }
    }

    fn code_at(&self, address: RpcH160, num: Trailing<BlockNumber>) -> BoxFuture<Bytes> {
        measure_counter_inc!("getCode");
        let address: Address = RpcH160::into(address);
        let num = num.unwrap_or_default();

        info!("eth_getCode(address: {:?}, number: {:?})", address, num);

        Box::new(
            self.client
                .code(&address, Self::get_block_id(num))
                .map(|code| code.map_or_else(Bytes::default, Bytes::new))
                .map_err(|_error| errors::state_pruned()),
        )
    }

    fn block_by_hash(&self, hash: RpcH256, include_txs: bool) -> BoxFuture<Option<RichBlock>> {
        measure_counter_inc!("getBlockByHash");
        info!(
            "eth_getBlockByHash(hash: {:?}, full: {:?})",
            hash, include_txs
        );
        self.rich_block(BlockId::Hash(hash.into()).into(), include_txs)
    }

    fn block_by_number(&self, num: BlockNumber, include_txs: bool) -> BoxFuture<Option<RichBlock>> {
        measure_counter_inc!("getBlockByNumber");
        info!(
            "eth_getBlockByNumber(number: {:?}, full: {:?})",
            num, include_txs
        );
        self.rich_block(num.into(), include_txs)
    }

    fn transaction_by_hash(&self, hash: RpcH256) -> BoxFuture<Option<RpcTransaction>> {
        measure_counter_inc!("getTransactionByHash");
        info!("eth_getTransactionByHash(hash: {:?})", hash);
        let hash: H256 = hash.into();
        let tx = try_bf!(self.transaction(PendingTransactionId::Hash(hash)));
        Box::new(future::ok(tx))
    }

    fn transaction_by_block_hash_and_index(
        &self,
        hash: RpcH256,
        index: Index,
    ) -> BoxFuture<Option<RpcTransaction>> {
        measure_counter_inc!("getTransactionByBlockHashAndIndex");
        info!(
            "eth_getTransactionByBlockHashAndIndex(hash: {:?}, index: {:?})",
            hash, index
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
        measure_counter_inc!("getTransactionByBlockNumberAndIndex");
        info!(
            "eth_getTransactionByBlockNumberAndIndex(number: {:?}, index: {:?})",
            num, index
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
        measure_counter_inc!("getTransactionReceipt");
        let hash: H256 = hash.into();
        info!("eth_getTransactionReceipt(hash: {:?})", hash);
        let receipt = self.client.transaction_receipt(hash);
        Box::new(future::ok(receipt.map(Into::into)))
    }

    fn uncle_by_block_hash_and_index(
        &self,
        _hash: RpcH256,
        _index: Index,
    ) -> BoxFuture<Option<RichBlock>> {
        measure_counter_inc!("getUncleByBlockHashAndIndex");
        // we dont' have uncles
        Box::new(future::ok(None))
    }

    fn uncle_by_block_number_and_index(
        &self,
        _num: BlockNumber,
        _index: Index,
    ) -> BoxFuture<Option<RichBlock>> {
        measure_counter_inc!("getUncleByBlockNumberAndIndex");
        // we dont' have uncles
        Box::new(future::ok(None))
    }

    fn compilers(&self) -> Result<Vec<String>> {
        measure_counter_inc!("getCompilers");
        Err(errors::deprecated(
            "Compilation functionality is deprecated.".to_string(),
        ))
    }

    fn logs(&self, filter: Filter) -> BoxFuture<Vec<RpcLog>> {
        measure_counter_inc!("getLogs");
        info!("eth_getLogs(filter: {:?})", filter);
        let filter: EthcoreFilter = filter.into();
        let logs = self.client
            .logs(filter.clone())
            .into_iter()
            .map(From::from)
            .collect::<Vec<RpcLog>>();
        let logs = limit_logs(logs, filter.limit);
        Box::new(future::ok(logs))
    }

    fn work(&self, _no_new_work_timeout: Trailing<u64>) -> Result<Work> {
        measure_counter_inc!("getWork");
        Err(errors::unimplemented(None))
    }

    fn submit_work(&self, _nonce: RpcH64, _pow_hash: RpcH256, _mix_hash: RpcH256) -> Result<bool> {
        measure_counter_inc!("submitWork");
        Err(errors::unimplemented(None))
    }

    fn submit_hashrate(&self, _rate: RpcU256, _id: RpcH256) -> Result<bool> {
        measure_counter_inc!("submitHashrate");
        Err(errors::unimplemented(None))
    }

    fn send_raw_transaction(&self, raw: Bytes) -> BoxFuture<RpcH256> {
        measure_counter_inc!("sendRawTransaction");
        measure_histogram_timer!("sendRawTransaction_time");
        if log_enabled!(log::Level::Debug) {
            debug!("eth_sendRawTransaction(data: {:?})", raw);
        } else {
            info!("eth_sendRawTransaction(data: ...)");
        }

        Box::new(
            self.client
                .send_raw_transaction(raw.into())
                .map(Into::into)
                .map_err(errors::execution),
        )
    }

    fn submit_transaction(&self, raw: Bytes) -> BoxFuture<RpcH256> {
        measure_counter_inc!("submitTransaction");
        info!("eth_submitTransaction(data: {:?})", raw);
        self.send_raw_transaction(raw)
    }

    fn call(
        &self,
        meta: Self::Metadata,
        request: CallRequest,
        num: Trailing<BlockNumber>,
    ) -> BoxFuture<Bytes> {
        measure_counter_inc!("call");
        measure_histogram_timer!("call_time");
        let num = num.unwrap_or_default();

        info!("eth_call(request: {:?}, number: {:?})", request, num);

        let request = CallRequest::into(request);
        let signed = try_bf!(fake_sign::sign_call(request, meta.is_dapp()));
        let result = self.client.call(&signed, Self::get_block_id(num));
        Box::new(future::done(
            result
                .map_err(errors::call)
                .and_then(|executed| match executed.exception {
                    Some(ref exception) => Err(errors::vm(exception, &executed.output)),
                    None => Ok(executed),
                })
                .map(|b| b.output.into()),
        ))
    }

    fn estimate_gas(
        &self,
        meta: Self::Metadata,
        request: CallRequest,
        num: Trailing<BlockNumber>,
    ) -> BoxFuture<RpcU256> {
        measure_counter_inc!("estimateGas");
        measure_histogram_timer!("estimateGas_time");
        let num = num.unwrap_or_default();

        info!("eth_estimateGas(request: {:?}, number: {:?})", request, num);

        let request = CallRequest::into(request);
        let signed = try_bf!(fake_sign::sign_call(request, meta.is_dapp()));
        let result = self.client.estimate_gas(&signed, Self::get_block_id(num));
        Box::new(future::done(result.map(Into::into).map_err(errors::call)))
    }

    fn compile_lll(&self, _: String) -> Result<Bytes> {
        measure_counter_inc!("compileLLL");
        Err(errors::deprecated(
            "Compilation of LLL via RPC is deprecated".to_string(),
        ))
    }

    fn compile_serpent(&self, _: String) -> Result<Bytes> {
        measure_counter_inc!("compileSerpent");
        Err(errors::deprecated(
            "Compilation of Serpent via RPC is deprecated".to_string(),
        ))
    }

    fn compile_solidity(&self, _: String) -> Result<Bytes> {
        measure_counter_inc!("compileSolidity");
        Err(errors::deprecated(
            "Compilation of Solidity via RPC is deprecated".to_string(),
        ))
    }
}
