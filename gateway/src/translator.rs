//! Translator which translates between an Ekiden chain running the runtime-ethereum
//! runtime and an Ethereum chain exposed to clients.
use std::{collections::BTreeMap, sync::Arc};

use ekiden_client::{
    transaction::{
        snapshot::{BlockSnapshot, TransactionSnapshot},
        Query, QueryCondition, ROUND_LATEST, TAG_BLOCK_HASH,
    },
    BoxFuture,
};
use ekiden_runtime::{
    common::{cbor, crypto::hash::Hash, logger::get_logger},
    storage::MKVS,
    transaction::types::{TxnCall, TxnOutput},
};
use ethcore::{
    error::CallError,
    executive::{contract_address, Executed, Executive, TransactOptions},
    filter::Filter,
    log_entry::{LocalizedLogEntry, LogEntry},
    receipt::{LocalizedReceipt, TransactionOutcome},
    state::State,
    transaction::{Action, LocalizedTransaction, SignedTransaction, UnverifiedTransaction},
    types::ids::BlockId,
    vm::EnvInfo,
};
use ethereum_types::{H256, H64, U256};
use failure::{format_err, Error, Fallible};
use futures::{future, prelude::*};
use hash::KECCAK_EMPTY_LIST_RLP;
use io_context::Context;
use lazy_static::lazy_static;
use parity_rpc::v1::types::{
    Block as EthRpcBlock, BlockTransactions as EthRpcBlockTransactions, Header as EthRpcHeader,
    RichBlock as EthRpcRichBlock, RichHeader as EthRpcRichHeader, Transaction as EthRpcTransaction,
};
use runtime_ethereum_api::{ExecutionResult, TransactionError, METHOD_TX};
use runtime_ethereum_common::{
    genesis, parity::NullBackend, TAG_ETH_LOG_ADDRESS, TAG_ETH_LOG_TOPICS, TAG_ETH_TX_HASH,
};

use serde_bytes::ByteBuf;
use slog::{error, Logger};
use tokio_threadpool::{Builder as ThreadPoolBuilder, ThreadPool};

use crate::EthereumRuntimeClient;

/// Translator that enables exposing the runtime-ethereum runtime on Ekiden as an
/// Ethereum chain.
pub struct Translator {
    logger: Logger,
    client: Arc<EthereumRuntimeClient>,
    gas_price: U256,
    simulator_pool: Arc<ThreadPool>,
}

impl Translator {
    /// Create new translator.
    pub fn new(client: EthereumRuntimeClient, gas_price: U256) -> Self {
        Self {
            logger: get_logger("gateway/translator"),
            client: Arc::new(client),
            gas_price,
            simulator_pool: Arc::new(
                ThreadPoolBuilder::new()
                    .name_prefix("simulator-pool-")
                    .build(),
            ),
        }
    }

    /// Gas price.
    pub fn gas_price(&self) -> U256 {
        self.gas_price
    }

    /// Retrieve an Ethereum block given a block identifier.
    pub fn get_block(
        &self,
        id: BlockId,
    ) -> impl Future<Item = Option<EthereumBlock>, Error = Error> {
        let block: BoxFuture<Option<EthereumBlock>> = match id {
            BlockId::Hash(hash) => Box::new(self.get_block_by_hash(hash)),
            BlockId::Number(round) => Box::new(self.get_block_by_round(round)),
            BlockId::Latest => Box::new(self.get_latest_block().map(Some)),
            BlockId::Earliest => Box::new(self.get_block_by_round(0)),
        };

        block
    }

    /// Retrieve an Ethereum block given a block identifier.
    ///
    /// If the block is not found it returns an error.
    pub fn get_block_unwrap(
        &self,
        id: BlockId,
    ) -> impl Future<Item = EthereumBlock, Error = Error> {
        self.get_block(id).and_then(|blk| match blk {
            Some(blk) => Ok(blk),
            None => Err(format_err!("block not found")),
        })
    }

    /// Retrieve the latest Ethereum block.
    pub fn get_latest_block(&self) -> impl Future<Item = EthereumBlock, Error = Error> {
        let client = self.client.clone();
        self.client
            .txn_client()
            .get_latest_block()
            .map(|snapshot| EthereumBlock::new(snapshot, client))
    }

    /// Retrieve a specific Ethereum block, identified by its round number.
    pub fn get_block_by_round(
        &self,
        round: u64,
    ) -> impl Future<Item = Option<EthereumBlock>, Error = Error> {
        let client = self.client.clone();
        self.client
            .txn_client()
            .get_block(round)
            .map(|snapshot| snapshot.map(|snapshot| EthereumBlock::new(snapshot, client)))
    }

    /// Retrieve a specific Ethereum block, identified by its block hash.
    pub fn get_block_by_hash(
        &self,
        hash: H256,
    ) -> impl Future<Item = Option<EthereumBlock>, Error = Error> {
        let client = self.client.clone();
        self.client
            .txn_client()
            .query_block(TAG_BLOCK_HASH, hash)
            .map(|snapshot| snapshot.map(|snapshot| EthereumBlock::new(snapshot, client)))
    }

    /// Retrieve a specific Ethereum transaction, identified by its transaction hash.
    pub fn get_txn_by_hash(
        &self,
        hash: H256,
    ) -> impl Future<Item = Option<EthereumTransaction>, Error = Error> {
        self.client
            .txn_client()
            .query_txn(TAG_ETH_TX_HASH, hash)
            .map(|txn| txn.map(EthereumTransaction::new))
    }

    /// Retrieve a specific Ethereum transaction, identified by the block round and
    /// transaction index within the block.
    pub fn get_txn_by_round_and_index(
        &self,
        round: u64,
        index: u32,
    ) -> impl Future<Item = Option<EthereumTransaction>, Error = Error> {
        self.client
            .txn_client()
            .get_txn(round, index)
            .map(|txn| txn.map(EthereumTransaction::new))
    }

    /// Retrieve a specific Ethereum transaction, identified by the block hash and
    /// transaction index within the block.
    pub fn get_txn_by_block_hash_and_index(
        &self,
        block_hash: H256,
        index: u32,
    ) -> impl Future<Item = Option<EthereumTransaction>, Error = Error> {
        self.client
            .txn_client()
            .get_txn_by_block_hash(Hash::from(block_hash.as_ref() as &[u8]), index)
            .map(|txn| txn.map(EthereumTransaction::new))
    }

    /// Retrieve a specific Ethereum transaction, identified by a block identifier
    /// and transaction index within the block.
    pub fn get_txn(
        &self,
        id: BlockId,
        index: u32,
    ) -> impl Future<Item = Option<EthereumTransaction>, Error = Error> {
        let txn: BoxFuture<Option<EthereumTransaction>> = match id {
            BlockId::Hash(hash) => Box::new(self.get_txn_by_block_hash_and_index(hash, index)),
            BlockId::Number(round) => Box::new(self.get_txn_by_round_and_index(round, index)),
            BlockId::Latest => Box::new(self.get_txn_by_round_and_index(ROUND_LATEST, index)),
            BlockId::Earliest => Box::new(self.get_txn_by_round_and_index(0, index)),
        };

        txn
    }

    /// Submit a raw Ethereum transaction to the chain.
    pub fn send_raw_transaction(&self, raw: Vec<u8>) -> BoxFuture<(H256, ExecutionResult)> {
        // TODO: Perform more checks.
        let decoded: UnverifiedTransaction = match rlp::decode(&raw) {
            Ok(decoded) => decoded,
            Err(err) => return Box::new(future::err(err.into())),
        };

        // If we get a BlockGasLimitReached error, retry up to 5 times.
        const MAX_RETRIES: usize = 5;

        Box::new(future::loop_fn(
            (
                MAX_RETRIES,
                self.client.clone(),
                ByteBuf::from(raw),
                decoded,
            ),
            move |(retries, client, payload, decoded)| {
                client
                    .tx(payload.clone())
                    .then(move |maybe_result| match maybe_result {
                        Ok(result) => Ok(future::Loop::Break((decoded.hash(), result))),
                        Err(err) => {
                            if let Some(txn_err) = err.downcast_ref::<TransactionError>() {
                                if let TransactionError::BlockGasLimitReached = txn_err {
                                    if retries == 0 {
                                        return Err(err);
                                    }
                                    let retries = retries - 1;
                                    return Ok(future::Loop::Continue((
                                        retries, client, payload, decoded,
                                    )));
                                }
                            }
                            Err(err)
                        }
                    })
            },
        ))
    }

    /// Simulate a transaction against a given block.
    ///
    /// The simulated transaction is executed in a dedicated thread pool to
    /// avoid blocking I/O processing.
    ///
    /// # Notes
    ///
    /// Confidential contracts are not supported.
    pub fn simulate_transaction(
        &self,
        transaction: SignedTransaction,
        id: BlockId,
    ) -> impl Future<Item = Executed, Error = CallError> {
        let simulator_pool = self.simulator_pool.clone();

        self.get_block(id)
            .map_err(|_| CallError::StateCorrupt)
            .and_then(|blk| match blk {
                Some(blk) => Ok(blk),
                None => Err(CallError::StatePruned),
            })
            .and_then(move |blk| {
                // Execute simulation in a dedicated thread pool to avoid blocking
                // I/O processing with simulations.
                simulator_pool.spawn_handle(future::lazy(move || {
                    let mut state = blk.state().map_err(|_| CallError::StateCorrupt)?;
                    let env_info = EnvInfo {
                        number: blk.snapshot.block.header.round + 1,
                        author: Default::default(),
                        timestamp: blk.snapshot.block.header.timestamp,
                        difficulty: Default::default(),
                        // TODO: Get 256 last hashes.
                        last_hashes: Arc::new(vec![blk
                            .snapshot
                            .block
                            .header
                            .previous_hash
                            .as_ref()
                            .into()]),
                        gas_used: Default::default(),
                        gas_limit: U256::max_value(),
                    };
                    let machine = genesis::SPEC.engine.machine();
                    let options = TransactOptions::with_no_tracing()
                        .dont_check_nonce()
                        .save_output_from_contract();

                    Ok(Executive::new(&mut state, &env_info, machine)
                        .transact_virtual(&transaction, options)?)
                }))
            })
    }

    /// Estimates gas against a given block.
    ///
    /// Uses `simulate_transaction` internally.
    ///
    /// # Notes
    ///
    /// Confidential contracts are not supported.
    pub fn estimate_gas(
        &self,
        transaction: SignedTransaction,
        id: BlockId,
    ) -> impl Future<Item = U256, Error = CallError> {
        self.simulate_transaction(transaction, id)
            .map(|executed| executed.gas_used + executed.refunded)
    }

    /// Looks up logs based on the given filter.
    pub fn logs(
        &self,
        filter: Filter,
    ) -> impl Future<Item = Vec<LocalizedLogEntry>, Error = Error> {
        // Resolve starting and ending blocks.
        let client = self.client.clone();
        let blocks = future::join_all(vec![
            Box::new(self.get_block_unwrap(filter.from_block)) as BoxFuture<EthereumBlock>,
            Box::new(self.get_block_unwrap(filter.to_block).and_then(move |blk| {
                client
                    .txn_client()
                    .wait_block_indexed(blk.snapshot.block.header.round)
                    .map(move |()| blk)
            })),
        ]);

        // Look up matching transactions.
        let f = filter.clone();
        let client = self.client.clone();
        let txns = blocks.and_then(move |blks| {
            client.txn_client().query_txns(Query {
                round_min: blks[0].snapshot.block.header.round,
                round_max: blks[1].snapshot.block.header.round,
                conditions: {
                    let mut c = vec![];
                    // Transaction must emit logs for any of the given addresses.
                    if let Some(ref addresses) = filter.address {
                        c.push(QueryCondition {
                            key: TAG_ETH_LOG_ADDRESS.to_vec(),
                            values: addresses
                                .iter()
                                .map(|x| <[u8]>::as_ref(x).to_vec().into())
                                .collect(),
                        });
                    }
                    // Transaction must emit logs for all of the given topics.
                    c.extend(
                        filter
                            .topics
                            .iter()
                            .zip(TAG_ETH_LOG_TOPICS.iter())
                            .take(4)
                            .filter_map(|(topic, tag)| {
                                topic.as_ref().map(|topic| QueryCondition {
                                    key: tag.to_vec(),
                                    values: topic
                                        .iter()
                                        .map(|x| <[u8]>::as_ref(&x).to_vec().into())
                                        .collect(),
                                })
                            }),
                    );

                    c
                },
                limit: filter.limit.map(|l| l as u64).unwrap_or_default(),
            })
        });

        // Decode logs from resulting transactions.
        let filter = f;
        let logger = self.logger.clone();
        let logs = txns
            .map(move |txns| {
                txns.into_iter().flat_map(|txn| {
                // This should not happen as such transactions should not emit tags.
                if txn.input.method != METHOD_TX {
                    error!(logger, "Query returned non-ethereum transaction";
                        "method" => txn.input.method,
                    );
                    return vec![];
                }

                // We know that arguments are raw Ethereum transaction bytes.
                let raw: ByteBuf = match cbor::from_value(txn.input.args.clone()) {
                    Ok(raw) => raw,
                    Err(err) => {
                        error!(logger, "Error while decoding ethereum transaction input";
                            "err" => ?err,
                        );
                        return vec![];
                    }
                };
                let eth_tx: UnverifiedTransaction = match rlp::decode(&raw) {
                    Ok(tx) => tx,
                    Err(err) => {
                        error!(logger, "Error while decoding ethereum transaction input";
                            "err" => ?err,
                        );
                        return vec![];
                    }
                };

                let transaction_hash = eth_tx.hash();
                let transaction_index = txn.index as usize;
                let block_hash = txn.block_snapshot.block_hash.as_ref().into();
                let block_number = txn.block_snapshot.block.header.round;

                // Decode transaction output.
                match txn.output {
                    TxnOutput::Success(value) => {
                        // We know that output is ExecutionResult.
                        let result: ExecutionResult = match cbor::from_value(value) {
                            Ok(result) => result,
                            Err(err) => {
                                error!(logger, "Error while decoding ethereum transaction output";
                                    "err" => ?err,
                                );
                                return vec![];
                            }
                        };

                        result
                            .logs
                            .into_iter()
                            .enumerate()
                            .filter_map(|(i, e)| {
                                let entry = LogEntry {
                                    address: e.address,
                                    topics: e.topics,
                                    data: e.data,
                                };
                                if !filter.matches(&entry) {
                                    return None;
                                }

                                Some(LocalizedLogEntry {
                                    entry,
                                    block_hash,
                                    block_number,
                                    transaction_hash,
                                    transaction_index,
                                    log_index: i,
                                    transaction_log_index: i,
                                })
                            })
                            .collect()
                    }
                    _ => vec![],
                }
            }).collect()
            })
            .and_then(|logs: Vec<LocalizedLogEntry>| {
                let mut logs = logs;
                logs.sort_by(|a, b| a.block_number.partial_cmp(&b.block_number).unwrap());
                future::ok(logs)
            });

        Box::new(logs)
    }
}

/// A wrapper that exposes an Ekiden transaction against runtime-ethereum
/// as an Ethereum transaction.
pub struct EthereumTransaction {
    snapshot: TransactionSnapshot,
}

impl EthereumTransaction {
    /// Create a new Ethereum transaction from an Ekiden transaction snapshot.
    pub fn new(snapshot: TransactionSnapshot) -> Self {
        Self { snapshot }
    }

    /// Retrieve the (localized) Ethereum transaction input.
    pub fn transaction(&self) -> Fallible<LocalizedTransaction> {
        // Validate method.
        if self.snapshot.input.method != METHOD_TX {
            return Err(format_err!("not an Ethereum transaction"));
        }

        // We know that arguments are raw Ethereum transaction bytes.
        let raw: ByteBuf = cbor::from_value(self.snapshot.input.args.clone())?;
        let signed: UnverifiedTransaction = rlp::decode(&raw)?;

        Ok(LocalizedTransaction {
            signed,
            block_number: self.snapshot.block_snapshot.block.header.round,
            block_hash: self.snapshot.block_snapshot.block_hash.as_ref().into(),
            transaction_index: self.snapshot.index as usize,
            cached_sender: None,
        })
    }

    /// Retrieve the (localized) Ethereum transaction output (receipt).
    pub fn receipt(&self) -> Fallible<LocalizedReceipt> {
        match self.snapshot.output {
            TxnOutput::Success(ref value) => {
                // We know that output is ExecutionResult.
                let result: ExecutionResult = cbor::from_value(value.clone())?;
                // Decode input transaction.
                let mut tx = self.transaction()?;

                let transaction_hash = tx.hash();
                let transaction_index = tx.transaction_index;
                let block_hash = tx.block_hash;
                let block_number = tx.block_number;

                Ok(LocalizedReceipt {
                    transaction_hash,
                    transaction_index,
                    block_hash,
                    block_number,
                    cumulative_gas_used: result.cumulative_gas_used,
                    gas_used: result.gas_used,
                    contract_address: match tx.action {
                        Action::Call(_) => None,
                        Action::Create => Some(
                            contract_address(
                                genesis::SPEC.engine.create_address_scheme(block_number),
                                &tx.sender(),
                                &tx.nonce,
                                &tx.data,
                            )
                            .0,
                        ),
                    },
                    logs: result
                        .logs
                        .into_iter()
                        .enumerate()
                        .map(|(i, e)| LocalizedLogEntry {
                            entry: LogEntry {
                                address: e.address,
                                topics: e.topics,
                                data: e.data,
                            },
                            block_hash,
                            block_number,
                            transaction_hash,
                            transaction_index,
                            log_index: i,
                            transaction_log_index: i,
                        })
                        .collect(),
                    log_bloom: result.log_bloom,
                    outcome: TransactionOutcome::StatusCode(result.status_code),
                })
            }
            TxnOutput::Error(_) => Err(format_err!("receipt not available")),
        }
    }
}

/// A wrapper that exposes an Ekiden block generated by runtime-ethereum
/// as an Ethereum block.
pub struct EthereumBlock {
    snapshot: BlockSnapshot,
    client: Arc<EthereumRuntimeClient>,
}

impl EthereumBlock {
    /// Create a new Ethereum block from an Ekiden block snapshot.
    pub fn new(snapshot: BlockSnapshot, client: Arc<EthereumRuntimeClient>) -> Self {
        Self { snapshot, client }
    }

    /// Ethereum block number.
    pub fn number(&self) -> U256 {
        self.snapshot.block.header.round.into()
    }

    /// Ethereum block number as an u64.
    pub fn number_u64(&self) -> u64 {
        self.snapshot.block.header.round
    }

    /// Block hash.
    pub fn hash(&self) -> H256 {
        self.snapshot.block_hash.as_ref().into()
    }

    /// Ethereum state snapshot at given block.
    pub fn state(&self) -> Fallible<State<NullBackend>> {
        Ok(State::from_existing(
            Box::new(BlockSnapshotMKVS(self.snapshot.clone())),
            NullBackend,
            U256::zero(),       /* account_start_nonce */
            Default::default(), /* factories */
            None,               /* confidential_ctx */
        )?)
    }

    /// Raw Ekiden transactions in a block corresponding to Ethereum transactions.
    pub fn raw_transactions(
        &self,
    ) -> impl Future<Item = impl Iterator<Item = TxnCall>, Error = Error> {
        self.client
            .txn_client()
            .get_transactions(
                self.snapshot.block.header.round,
                self.snapshot.block.header.io_root,
            )
            .map(|txns| {
                txns.0.into_iter().filter_map(|txn| {
                    let txn: TxnCall = cbor::from_slice(&txn).ok()?;
                    if txn.method != METHOD_TX {
                        return None;
                    }

                    Some(txn)
                })
            })
    }

    // Ethereum transactions contained in the block.
    pub fn transactions(
        &self,
    ) -> impl Future<Item = impl Iterator<Item = UnverifiedTransaction>, Error = Error> {
        self.raw_transactions().and_then(|txns| {
            Ok(txns.filter_map(|txn| {
                let raw: ByteBuf = cbor::from_value(txn.args).ok()?;
                let signed: UnverifiedTransaction = rlp::decode(&raw).ok()?;

                Some(signed)
            }))
        })
    }

    /// Retrieve an Ethereum header with additional metadata.
    pub fn rich_header(&self) -> EthRpcRichHeader {
        let header = self.snapshot.block.header.clone();
        let block_hash = self.snapshot.block_hash;

        // Generate header metadata.
        EthRpcRichHeader {
            inner: EthRpcHeader {
                hash: Some(block_hash.as_ref().into()),
                size: None,
                parent_hash: header.previous_hash.as_ref().into(),
                uncles_hash: KECCAK_EMPTY_LIST_RLP.into(), /* empty list */
                author: Default::default(),
                miner: Default::default(),
                state_root: header.state_root.as_ref().into(),
                transactions_root: Default::default(),
                receipts_root: Default::default(),
                number: Some(header.round.into()),
                // TODO: Gas used.
                gas_used: Default::default(),
                // TODO: Gas limit.
                gas_limit: Default::default(),
                // TODO: Logs bloom.
                logs_bloom: Default::default(),
                timestamp: header.timestamp.into(),
                difficulty: Default::default(),
                seal_fields: vec![],
                extra_data: Default::default(),
            },
            extra_info: {
                lazy_static! {
                    // Dummy PoW-related block extras.
                    static ref EXTRA_INFO: BTreeMap<String, String> = {
                        let mut map = BTreeMap::new();
                        map.insert("mixHash".into(), format!("0x{:x}", H256::default()));
                        map.insert("nonce".into(), format!("0x{:x}", H64::default()));
                        map
                    };
                }

                EXTRA_INFO.clone()
            },
        }
    }

    /// Retrieve an Ethereum block with additional metadata.
    pub fn rich_block(
        &self,
        include_txns: bool,
    ) -> impl Future<Item = EthRpcRichBlock, Error = Error> {
        let header = self.snapshot.block.header.clone();
        let block_hash = self.snapshot.block_hash;
        let rich_header = self.rich_header();

        self.transactions().and_then(move |txns| {
            // Either include full localized transactions or just hashes.
            let transactions = if include_txns {
                EthRpcBlockTransactions::Full(
                    txns.enumerate()
                        .map(|(i, txn)| {
                            EthRpcTransaction::from_localized(
                                LocalizedTransaction {
                                    signed: txn,
                                    block_number: header.round,
                                    block_hash: block_hash.as_ref().into(),
                                    transaction_index: i,
                                    cached_sender: None,
                                },
                                genesis::SPEC.params().eip86_transition,
                            )
                        })
                        .collect(),
                )
            } else {
                EthRpcBlockTransactions::Hashes(txns.map(|txn| txn.hash().into()).collect())
            };

            // Generate block metadata.
            Ok(EthRpcRichBlock {
                inner: EthRpcBlock {
                    hash: rich_header.hash.clone(),
                    size: rich_header.size,
                    parent_hash: rich_header.parent_hash.clone(),
                    uncles_hash: rich_header.uncles_hash.clone(),
                    author: rich_header.author.clone(),
                    miner: rich_header.miner.clone(),
                    state_root: rich_header.state_root.clone(),
                    transactions_root: rich_header.transactions_root.clone(),
                    receipts_root: rich_header.receipts_root.clone(),
                    number: rich_header.number,
                    gas_used: rich_header.gas_used,
                    gas_limit: rich_header.gas_limit,
                    logs_bloom: Some(rich_header.logs_bloom.clone()),
                    timestamp: rich_header.timestamp,
                    difficulty: rich_header.difficulty,
                    total_difficulty: None,
                    seal_fields: rich_header.seal_fields.clone(),
                    uncles: vec![],
                    transactions,
                    extra_data: rich_header.extra_data.clone(),
                },
                extra_info: rich_header.extra_info.clone(),
            })
        })
    }
}

#[derive(Clone)]
struct BlockSnapshotMKVS(BlockSnapshot);

impl ethcore::mkvs::MKVS for BlockSnapshotMKVS {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        // TODO: Use proper context.
        MKVS::get(&self.0, Context::background(), key)
    }

    fn insert(&mut self, key: &[u8], value: &[u8]) -> Option<Vec<u8>> {
        // TODO: Use proper context.
        MKVS::insert(&mut self.0, Context::background(), key, value)
    }

    fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        // TODO: Use proper context.
        MKVS::remove(&mut self.0, Context::background(), key)
    }

    fn boxed_clone(&self) -> Box<dyn ethcore::mkvs::MKVS> {
        Box::new(self.clone())
    }
}
