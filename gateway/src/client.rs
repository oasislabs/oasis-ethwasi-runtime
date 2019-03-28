use std::{
    marker::{Send, Sync},
    sync::{Arc, Mutex, RwLock, Weak},
    vec::Vec,
};

use bytes::Bytes;
use common_types::log_entry::LocalizedLogEntry;
#[cfg(not(test))]
use ekiden_client::transaction::snapshot::BlockSnapshot;
use ekiden_client::BoxFuture;
use ekiden_runtime::{common::logger::get_logger, storage::MKVS};
use ethcore::{
    blockchain::{BlockProvider, TransactionAddress},
    encoded,
    engines::EthEngine,
    error::{CallError, ExecutionError},
    executive::{contract_address, Executed, Executive, TransactOptions},
    filter::{Filter as EthcoreFilter, TxEntry as EthTxEntry},
    header::BlockNumber,
    ids::{BlockId, TransactionId},
    receipt::LocalizedReceipt,
    rlp,
    spec::Spec,
    transaction::UnverifiedTransaction,
    vm::{EnvInfo, LastHashes, OasisContract},
};
use ethereum_types::{Address, H256, U256};
use failure::{format_err, Error, Fallible};
use futures::{future, prelude::*};
use lazy_static::lazy_static;
use prometheus::{
    histogram_opts, opts, register_counter, register_histogram, register_int_counter, Histogram,
    IntCounter,
};
use runtime_ethereum_api::TransactionRequest;
use runtime_ethereum_common::State as EthState;
use slog::{debug, error, info, Logger};
use tokio::runtime::TaskExecutor;
use transaction::{Action, LocalizedTransaction, SignedTransaction};

#[cfg(test)]
use crate::test_helpers::{self, MockDb};
#[cfg(test)]
use crate::util;
use crate::{
    future_ext::block_on,
    state::{self, StateDb},
    util::from_block_id,
    EthereumRuntimeClient,
};

// Metrics.
lazy_static! {
    static ref RUNTIME_CALL_SUCCEEDED: IntCounter = register_int_counter!(
        "web3_gateway_runtime_call_succeeded",
        "Number of successful calls into the runtime"
    )
    .unwrap();
    static ref RUNTIME_CALL_FAILED: IntCounter = register_int_counter!(
        "web3_gateway_runtime_call_failed",
        "Number of failed calls into the runtime"
    )
    .unwrap();
    static ref READ_STATE_FAILED: IntCounter = register_int_counter!(
        "web3_gateway_read_state_failed",
        "Number of failed state reads"
    )
    .unwrap();
    static ref CONTRACT_CREATED: IntCounter = register_int_counter!(
        "web3_gateway_contract_created",
        "Number of create contract calls"
    )
    .unwrap();
    static ref ENC_CONTRACT_CREATED: IntCounter = register_int_counter!(
        "web3_gateway_confidential_contract_created",
        "Number of confidential create contract calls"
    )
    .unwrap();
    static ref LOG_FILTER_REJECTED: IntCounter = register_int_counter!(
        "web3_gateway_log_filter_rejected",
        "Number of rejected log filters"
    )
    .unwrap();
    static ref PUBSUB_NOTIFY_TIME: Histogram = register_histogram!(
        "web3_gateway_pubsub_notify_time",
        "Time it takes to dispatch pubsub notifications"
    )
    .unwrap();
}

/// Record runtime call outcome.
fn record_runtime_call_result<F, T>(logger: &Logger, call: &'static str, result: F) -> BoxFuture<T>
where
    T: 'static + Send,
    F: 'static + Future<Item = T, Error = Error> + Send,
{
    let logger = logger.clone();

    Box::new(result.then(move |result| {
        match result {
            Ok(_) => {
                RUNTIME_CALL_SUCCEEDED.inc();
            }
            Err(ref error) => {
                RUNTIME_CALL_FAILED.inc();
                error!(logger, "Runtime call failed"; "call" => call, "err" => ?error);
            }
        }

        result
    }))
}

/// An actor listening to chain events.
pub trait ChainNotify: Send + Sync {
    fn has_heads_subscribers(&self) -> bool;

    /// Notifies about new headers.
    fn notify_heads(&self, headers: &[encoded::Header]);

    /// Notifies about new log filter matches.
    fn notify_logs(&self, from_block: BlockId, to_block: BlockId);

    /// Notifies about a completed transaction.
    fn notify_completed_transaction(&self, entry: &EthTxEntry, output: Vec<u8>);
}

pub struct CheckedTransaction {
    pub from_address: Address,
    pub contract: Option<OasisContract>,
}

pub struct Client {
    logger: Logger,
    executor: TaskExecutor,
    client: Arc<EthereumRuntimeClient>,
    engine: Arc<EthEngine>,
    eip86_transition: u64,
    /// The most recent block for which we have sent notifications.
    notified_block_number: Mutex<BlockNumber>,
    listeners: Arc<RwLock<Vec<Weak<ChainNotify>>>>,
    gas_price: U256,
}

impl Client {
    pub fn new(
        executor: TaskExecutor,
        client: EthereumRuntimeClient,
        spec: &Spec,
        gas_price: U256,
    ) -> Self {
        let logger = get_logger("gateway/client");

        // Get current block number from db snapshot (or 0).
        debug!(logger, "Discovering latest block number");
        let current_block_number = block_on(&executor, client.txn_client().get_latest_block())
            .and_then(|snapshot| state::StateDb::new(Arc::new(snapshot.clone()), snapshot))
            .and_then(|maybe_db| Ok(maybe_db.map_or(0, |db| db.best_block_number())))
            .unwrap_or(0);

        debug!(logger, "Discovered latest block number"; "block_num" => current_block_number);

        Self {
            logger,
            executor,
            client: Arc::new(client),
            engine: spec.engine.clone(),
            eip86_transition: spec.params().eip86_transition,
            // start at current block
            notified_block_number: Mutex::new(current_block_number),
            listeners: Arc::new(RwLock::new(vec![])),
            gas_price: gas_price,
        }
    }

    /// A blockchain client for unit tests.
    #[cfg(test)]
    pub fn new_test_client() -> Self {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let spec = &util::load_spec();

        Self {
            logger: get_logger("gateway/client"),
            executor: runtime.executor(),
            client: Arc::new(test_helpers::new_test_runtime_client()),
            engine: spec.engine.clone(),
            eip86_transition: spec.params().eip86_transition,
            notified_block_number: Mutex::new(0),
            listeners: Arc::new(RwLock::new(vec![])),
            gas_price: U256::from(1_000_000_000),
        }
    }

    /// Notify listeners of new blocks.
    #[cfg(feature = "pubsub")]
    pub fn new_blocks(&self) {
        const MAX_HEADERS: u64 = 256;

        let mut last_block = self.notified_block_number.lock().unwrap();

        let _timer = PUBSUB_NOTIFY_TIME.start_timer();

        if let Some(db) = self.get_db_snapshot() {
            let current_block = db.best_block_number();
            if current_block > *last_block {
                self.notify(|listener| {
                    // optimization: only generate the list of headers if we have subscribers
                    if listener.has_heads_subscribers() {
                        // notify listeners of up to 256 most recent headers since last notification
                        let headers =
                            Self::headers_since(&db, *last_block + 1, current_block, MAX_HEADERS);
                        listener.notify_heads(&headers);
                    }

                    // notify log listeners of blocks last+1...current
                    listener.notify_logs(
                        BlockId::Number(*last_block + 1),
                        BlockId::Number(current_block),
                    );
                });

                // update last notified block
                *last_block = current_block;
            }
        }
    }

    /// Adds a new `ChainNotify` listener.
    pub fn add_listener(&self, listener: Weak<ChainNotify>) {
        self.listeners.write().unwrap().push(listener);
    }

    /// Notify `ChainNotify` listeners.
    fn notify<F: Fn(&ChainNotify)>(&self, f: F) {
        Client::notify_listeners(&self.listeners, f)
    }

    fn notify_listeners<F: Fn(&ChainNotify)>(
        listeners: &Arc<RwLock<Vec<Weak<ChainNotify>>>>,
        f: F,
    ) {
        for listener in &*listeners.read().unwrap() {
            if let Some(listener) = listener.upgrade() {
                f(&*listener)
            }
        }
    }

    /// Returns the BlockId corresponding to the larger block number.
    #[cfg(feature = "pubsub")]
    pub fn max_block_number(&self, id_a: BlockId, id_b: BlockId) -> BlockId {
        // first check if either is Latest
        if id_a == BlockId::Latest || id_b == BlockId::Latest {
            return BlockId::Latest;
        }

        // if either is Earliest, return the other
        if id_a == BlockId::Earliest {
            return id_b;
        }
        if id_b == BlockId::Earliest {
            return id_a;
        }

        // compare block numbers
        let num_a = match self.id_to_block_number(id_a) {
            Some(num) => num,
            None => return id_b,
        };
        let num_b = match self.id_to_block_number(id_b) {
            Some(num) => num,
            None => return id_a,
        };
        if num_a > num_b {
            id_a
        } else {
            id_b
        }
    }

    /// Returns the BlockId corresponding to the smaller block number.
    #[cfg(feature = "pubsub")]
    pub fn min_block_number(&self, id_a: BlockId, id_b: BlockId) -> BlockId {
        // first check if either is Earliest
        if id_a == BlockId::Earliest || id_b == BlockId::Earliest {
            return BlockId::Earliest;
        }

        // if either is Latest, return the other
        if id_a == BlockId::Latest {
            return id_b;
        }
        if id_b == BlockId::Latest {
            return id_a;
        }

        // compare block numbers
        let num_a = match self.id_to_block_number(id_a) {
            Some(num) => num,
            None => return id_b,
        };
        let num_b = match self.id_to_block_number(id_b) {
            Some(num) => num,
            None => return id_a,
        };
        if num_a < num_b {
            id_a
        } else {
            id_b
        }
    }

    /// Gas price
    pub fn gas_price(&self) -> U256 {
        self.gas_price.clone()
    }

    /// Block number at which EIP-86 transition occurs.
    /// https://github.com/ethereum/EIPs/blob/master/EIPS/eip-86.md
    pub fn eip86_transition(&self) -> u64 {
        self.eip86_transition
    }

    /// Returns a StateDb backed by an Ekiden db snapshot, or None when the
    /// blockchain database has not yet been initialized by the runtime.
    #[cfg(not(test))]
    fn get_db_snapshot(&self) -> Option<StateDb<BlockSnapshot>> {
        block_on(&self.executor, self.client.txn_client().get_latest_block())
            .and_then(|snapshot| state::StateDb::new(Arc::new(snapshot.clone()), snapshot))
            .unwrap_or_else(|err| {
                READ_STATE_FAILED.inc();
                error!(self.logger, "Could not get db snapshot"; "err" => ?err);

                None
            })
    }

    /// Returns a MockDb-backed StateDb for unit tests.
    #[cfg(test)]
    fn get_db_snapshot(&self) -> Option<StateDb<MockDb>> {
        let db = MockDb::new();
        StateDb::new(db.cas(), db).unwrap()
    }

    // block-related

    pub fn best_block_number(&self) -> BlockNumber {
        if let Some(db) = self.get_db_snapshot() {
            return db.best_block_number();
        }

        // Fall back to runtime call if database has not been initialized.
        // NOTE: We need to block on this call as making this method futures-aware
        //       would complicate the consumers a lot.
        block_on(
            &self.executor,
            record_runtime_call_result(
                &self.logger,
                "get_block_height",
                self.client
                    .get_block_height(false)
                    .map(|height| height.into()),
            ),
        )
        .unwrap_or_default()
    }

    pub fn block(&self, id: BlockId) -> BoxFuture<Option<encoded::Block>> {
        if let Some(db) = self.get_db_snapshot() {
            return Box::new(future::ok(
                Self::id_to_block_hash(&db, id).and_then(|h| db.block(&h)),
            ));
        }

        // Fall back to runtime call if database has not been initialized.
        record_runtime_call_result(
            &self.logger,
            "get_block",
            self.client
                .get_block(from_block_id(id))
                .map(|block| block.map(|block| encoded::Block::new(block))),
        )
    }

    pub fn block_hash(&self, id: BlockId) -> Option<H256> {
        if let BlockId::Hash(hash) = id {
            Some(hash)
        } else {
            if let Some(db) = self.get_db_snapshot() {
                match id {
                    BlockId::Hash(_hash) => unreachable!(),
                    BlockId::Number(number) => db.block_hash(number),
                    BlockId::Earliest => db.block_hash(0),
                    BlockId::Latest => db.best_block_hash(),
                }
            } else {
                None
            }
        }
    }

    fn id_to_block_number(&self, id: BlockId) -> Option<BlockNumber> {
        match id {
            BlockId::Latest => Some(self.best_block_number()),
            BlockId::Earliest => Some(0),
            BlockId::Number(num) => Some(num),
            BlockId::Hash(hash) => match self.get_db_snapshot() {
                Some(db) => db.block_number(&hash),
                None => None,
            },
        }
    }

    // transaction-related

    pub fn transaction(&self, id: TransactionId) -> Option<LocalizedTransaction> {
        if let Some(db) = self.get_db_snapshot() {
            let address = match id {
                TransactionId::Hash(ref hash) => db.transaction_address(hash),
                TransactionId::Location(id, index) => {
                    Self::id_to_block_hash(&db, id).map(|hash| TransactionAddress {
                        block_hash: hash,
                        index: index,
                    })
                }
            };
            address.and_then(|addr| db.transaction(&addr))
        } else {
            None
        }
    }

    pub fn transaction_receipt(&self, hash: H256) -> Option<LocalizedReceipt> {
        if let Some(db) = self.get_db_snapshot() {
            let address = db.transaction_address(&hash)?;
            let receipt = db.transaction_receipt(&address)?;
            let mut tx = db.transaction(&address)?;

            let transaction_hash = tx.hash();
            let block_hash = tx.block_hash;
            let block_number = tx.block_number;
            let transaction_index = tx.transaction_index;

            // Cumulative gas used by previous transactions in block.
            let prev_gas_used = if transaction_index > 0 {
                db.block_receipts(&block_hash)
                    .and_then(|br| br.receipts.into_iter().nth(transaction_index - 1))
                    .map_or(U256::from(0), |r| r.gas_used)
            } else {
                U256::from(0)
            };

            Some(LocalizedReceipt {
                transaction_hash: transaction_hash,
                transaction_index: transaction_index,
                block_hash: block_hash,
                block_number: block_number,
                cumulative_gas_used: receipt.gas_used,
                gas_used: receipt.gas_used - prev_gas_used,
                contract_address: match tx.action {
                    Action::Call(_) => None,
                    Action::Create => Some(
                        contract_address(
                            self.engine.create_address_scheme(block_number),
                            &tx.sender(),
                            &tx.nonce,
                            &tx.data,
                        )
                        .0,
                    ),
                },
                logs: receipt
                    .logs
                    .into_iter()
                    .enumerate()
                    .map(|(i, log)| LocalizedLogEntry {
                        entry: log,
                        block_hash: block_hash,
                        block_number: block_number,
                        transaction_hash: transaction_hash,
                        transaction_index: transaction_index,
                        transaction_log_index: i,
                        log_index: i,
                    })
                    .collect(),
                log_bloom: receipt.log_bloom,
                outcome: receipt.outcome,
            })
        } else {
            None
        }
    }

    fn id_to_block_hash<T>(db: &StateDb<T>, id: BlockId) -> Option<H256>
    where
        T: 'static + MKVS + Send + Sync,
    {
        match id {
            BlockId::Hash(hash) => Some(hash),
            BlockId::Number(number) => db.block_hash(number),
            BlockId::Earliest => db.block_hash(0),
            BlockId::Latest => db.best_block_hash(),
        }
    }

    /// Temporary mitigation for #397: returns false if filter's block range > 1000 blocks
    pub fn check_filter_range(&self, filter: EthcoreFilter) -> bool {
        const MAX_BLOCK_RANGE: u64 = 1000;

        let check_range = || {
            let db = self.get_db_snapshot()?;
            let from_hash = Self::id_to_block_hash(&db, filter.from_block)?;
            let from_number = db.block_number(&from_hash)?;
            let to_hash = Self::id_to_block_hash(&db, filter.to_block)?;
            let to_number = db.block_number(&to_hash)?;

            // Check block range
            if to_number > from_number {
                if to_number - from_number >= MAX_BLOCK_RANGE {
                    LOG_FILTER_REJECTED.inc();
                    error!(
                        self.logger,
                        "getLogs rejected block range";
                            "from_number" => from_number,
                            "to_number" => to_number,
                    );
                    return Some(false);
                }
            }

            Some(true)
        };

        check_range().unwrap_or(true)
    }

    pub fn logs(&self, filter: EthcoreFilter) -> Vec<LocalizedLogEntry> {
        if let Some(db) = self.get_db_snapshot() {
            let fetch_logs = || {
                let from_hash = Self::id_to_block_hash(&db, filter.from_block)?;
                let from_number = db.block_number(&from_hash)?;
                // NOTE: there appears to be a bug in parity with to_hash:
                // https://github.com/ekiden/parity/blob/master/ethcore/src/client/client.rs#L1856
                let to_hash = Self::id_to_block_hash(&db, filter.to_block)?;

                let blooms = filter.bloom_possibilities();
                let bloom_match = |header: &encoded::Header| {
                    blooms
                        .iter()
                        .any(|bloom| header.log_bloom().contains_bloom(bloom))
                };

                let (blocks, last_hash) = {
                    let mut blocks = Vec::new();
                    let mut current_hash = to_hash;

                    loop {
                        let header = db.block_header_data(&current_hash)?;
                        if bloom_match(&header) {
                            blocks.push(current_hash);
                        }

                        // Stop if `from` block is reached.
                        if header.number() <= from_number {
                            break;
                        }
                        current_hash = header.parent_hash();
                    }

                    blocks.reverse();
                    (blocks, current_hash)
                };

                // Check if we've actually reached the expected `from` block.
                if last_hash != from_hash || blocks.is_empty() {
                    return None;
                }

                Some(db.logs(blocks, |entry| filter.matches(entry), filter.limit))
            };

            fetch_logs().unwrap_or_default()
        } else {
            vec![]
        }
    }

    // account state-related

    pub fn balance(&self, address: &Address, id: BlockId) -> BoxFuture<U256> {
        match self.get_db_snapshot() {
            Some(db) => {
                if let Some(state) = db.get_ethstate_at(id) {
                    match state.balance(&address) {
                        Ok(balance) => Box::new(future::ok(balance)),
                        Err(err) => {
                            READ_STATE_FAILED.inc();
                            error!(self.logger, "Could not get balance from ethstate"; "err" => ?err);

                            Box::new(future::err(format_err!("Could not get balance")))
                        }
                    }
                } else {
                    Box::new(future::err(format_err!("Unknown block")))
                }
            }
            None => {
                // Fall back to runtime call if database has not been initialized.
                record_runtime_call_result(
                    &self.logger,
                    "get_account_balance",
                    self.client.get_account_balance(*address),
                )
            }
        }
    }

    pub fn code(&self, address: &Address, id: BlockId) -> BoxFuture<Option<Bytes>> {
        // TODO: differentiate between no account vs no code?
        match self.get_db_snapshot() {
            Some(db) => {
                if let Some(state) = db.get_ethstate_at(id) {
                    match state.code(&address) {
                        Ok(code) => Box::new(future::ok(code.map(|c| (&*c).clone()))),
                        Err(err) => {
                            READ_STATE_FAILED.inc();
                            error!(self.logger, "Could not get code from ethstate"; "err" => ?err);

                            Box::new(future::err(format_err!("Could not get code")))
                        }
                    }
                } else {
                    Box::new(future::err(format_err!("Unknown block")))
                }
            }
            None => {
                // Fall back to runtime call if database has not been initialized.
                record_runtime_call_result(
                    &self.logger,
                    "get_account_code",
                    self.client.get_account_code(*address),
                )
            }
        }
    }

    pub fn nonce(&self, address: &Address, id: BlockId) -> BoxFuture<U256> {
        match self.get_db_snapshot() {
            Some(db) => {
                if let Some(state) = db.get_ethstate_at(id) {
                    match state.nonce(&address) {
                        Ok(nonce) => Box::new(future::ok(nonce)),
                        Err(err) => {
                            READ_STATE_FAILED.inc();
                            error!(self.logger, "Could not get nonce from ethstate"; "err" => ?err);

                            Box::new(future::err(format_err!("Could not get nonce")))
                        }
                    }
                } else {
                    Box::new(future::err(format_err!("Unknown block")))
                }
            }
            None => {
                // Fall back to runtime call if database has not been initialized.
                record_runtime_call_result(
                    &self.logger,
                    "get_account_nonce",
                    self.client.get_account_nonce(*address),
                )
            }
        }
    }

    pub fn storage_at(&self, address: &Address, position: &H256, id: BlockId) -> BoxFuture<H256> {
        match self.get_db_snapshot() {
            Some(db) => {
                if let Some(state) = db.get_ethstate_at(id) {
                    match state.storage_at(address, position) {
                        Ok(val) => Box::new(future::ok(val)),
                        Err(err) => {
                            READ_STATE_FAILED.inc();
                            error!(self.logger, "Could not get storage from ethstate"; "err" => ?err);

                            Box::new(future::err(format_err!("Could not get storage")))
                        }
                    }
                } else {
                    Box::new(future::err(format_err!("Unknown block")))
                }
            }
            None => {
                // Fall back to runtime call if database has not been initialized.
                record_runtime_call_result(
                    &self.logger,
                    "get_storage_at",
                    self.client.get_storage_at((*address, *position)),
                )
            }
        }
    }

    pub fn storage_expiry(&self, address: &Address, id: BlockId) -> BoxFuture<u64> {
        match self.get_db_snapshot() {
            Some(db) => {
                if let Some(state) = db.get_ethstate_at(id) {
                    match state.storage_expiry(&address) {
                        Ok(timestamp) => Box::new(future::ok(timestamp)),
                        Err(err) => {
                            READ_STATE_FAILED.inc();
                            error!(self.logger, "Could not get storage expiry from ethstate"; "err" => ?err);

                            Box::new(future::err(format_err!("Could not get storage expiry")))
                        }
                    }
                } else {
                    Box::new(future::err(format_err!("Unknown block")))
                }
            }
            None => {
                // Fall back to runtime call if database has not been initialized.
                record_runtime_call_result(
                    &self.logger,
                    "get_storage_expiry",
                    self.client.get_storage_expiry(*address),
                )
            }
        }
    }

    fn last_hashes<T>(db: &StateDb<T>, parent_hash: &H256) -> Arc<LastHashes>
    where
        T: 'static + MKVS + Send + Sync,
    {
        let mut last_hashes = LastHashes::new();
        last_hashes.resize(256, H256::default());
        last_hashes[0] = parent_hash.clone();
        for i in 0..255 {
            match db.block_details(&last_hashes[i]) {
                Some(details) => {
                    last_hashes[i + 1] = details.parent.clone();
                }
                None => break,
            }
        }
        Arc::new(last_hashes)
    }

    /// Returns a vector of block headers from block numbers start...end (inclusive).
    /// Limited to the `max` most recent headers.
    fn headers_since<T>(
        db: &StateDb<T>,
        start: BlockNumber,
        end: BlockNumber,
        max: u64,
    ) -> Vec<encoded::Header>
    where
        T: 'static + MKVS + Send + Sync,
    {
        // limit to `max` headers
        let start = if end - start + 1 >= max {
            end - max + 1
        } else {
            start
        };

        let mut head = db
            .block_hash(end)
            .and_then(|hash| db.block_header_data(&hash))
            .expect("Invalid block number");

        let mut headers = Vec::with_capacity((end - start + 1) as usize);

        loop {
            headers.push(head.clone());
            if head.number() <= start {
                break;
            }
            head = db
                .block_header_data(&head.parent_hash())
                .expect("Chain is corrupt");
        }
        headers.reverse();
        headers
    }

    fn get_env_info<T>(db: &StateDb<T>) -> EnvInfo
    where
        T: 'static + MKVS + Send + Sync,
    {
        let parent = db
            .best_block_hash()
            .and_then(|hash| db.block_header_data(&hash))
            .expect("No best block");

        EnvInfo {
            // next block
            number: parent.number() + 1,
            author: Address::default(),
            timestamp: parent.timestamp(),
            difficulty: U256::zero(),
            last_hashes: Self::last_hashes(db, &parent.hash()),
            gas_used: U256::default(),
            gas_limit: U256::max_value(),
        }
    }

    // transaction-related
    pub fn call(
        &self,
        transaction: &SignedTransaction,
        id: BlockId,
    ) -> Result<Executed, CallError> {
        let db = match self.get_db_snapshot() {
            Some(db) => db,
            None => {
                error!(self.logger, "Could not get db snapshot");
                return Err(CallError::StateCorrupt);
            }
        };
        let mut state = match db.get_ethstate_at(id) {
            Some(state) => state,
            None => {
                error!(self.logger, "Could not get state snapshot");
                return Err(CallError::StateCorrupt);
            }
        };

        let env_info = Self::get_env_info(&db);
        let machine = self.engine.machine();
        let options = TransactOptions::with_no_tracing()
            .dont_check_nonce()
            .save_output_from_contract();
        let ret = Executive::new(&mut state, &env_info, machine)
            .transact_virtual(transaction, options)?;
        Ok(ret)
    }

    pub fn call_enc(&self, request: TransactionRequest, _id: BlockId) -> BoxFuture<Bytes> {
        record_runtime_call_result(
            &self.logger,
            "simulate_transaction",
            self.client
                .simulate_transaction(request)
                .and_then(|r| r.result.map_err(|error| format_err!("{}", error))),
        )
    }

    pub fn estimate_gas(&self, transaction: &SignedTransaction, id: BlockId) -> BoxFuture<U256> {
        let db = match self.get_db_snapshot() {
            Some(db) => db,
            None => {
                error!(self.logger, "Could not get db snapshot");
                return Box::new(future::err(format_err!("Could not estimate gas")));
            }
        };
        let state = match db.get_ethstate_at(id) {
            Some(state) => state,
            None => {
                error!(self.logger, "Could not get state snapshot");
                return Box::new(future::err(format_err!("Could not estimate gas")));
            }
        };

        // Extract contract deployment header.
        let oasis_contract = match state.oasis_contract(transaction) {
            Ok(contract) => contract,
            Err(error) => return Box::new(future::err(format_err!("{}", error))),
        };

        let confidential = oasis_contract.as_ref().map_or(false, |c| c.confidential);
        if confidential {
            self.confidential_estimate_gas(transaction)
        } else {
            let result = self._estimate_gas(transaction, db, state);
            Box::new(future::done(
                result
                    .map(Into::into)
                    .map_err(|error| format_err!("{}", error)),
            ))
        }
    }

    /// Estimates gas for a transaction calling a regular, non-confidential contract
    /// by running the transaction locally at the gateway.
    fn _estimate_gas<T: 'static + MKVS + Send + Sync>(
        &self,
        transaction: &SignedTransaction,
        db: StateDb<T>,
        mut state: EthState,
    ) -> Result<U256, CallError> {
        info!(self.logger, "estimating gas for a contract");

        let env_info = Self::get_env_info(&db);
        let machine = self.engine.machine();
        let options = TransactOptions::with_no_tracing()
            .dont_check_nonce()
            .save_output_from_contract();
        let ret = Executive::new(&mut state, &env_info, machine)
            .transact_virtual(transaction, options)?;

        match ret.exception {
            Some(err) => {
                let s = format!("{}", err);
                Err(CallError::Execution(ExecutionError::Internal(s)))
            }
            None => Ok(ret.gas_used + ret.refunded),
        }
    }

    /// Estimates gas for a transaction calling a confidential contract by sending
    /// the transaction through the scheduler to be run by the compute comittee.
    fn confidential_estimate_gas(&self, transaction: &SignedTransaction) -> BoxFuture<U256> {
        info!(self.logger, "estimating gas for a confidential contract");

        let to_addr = match transaction.action {
            Action::Create => None,
            Action::Call(to_addr) => Some(to_addr),
        };

        let request = TransactionRequest {
            nonce: Some(transaction.nonce),
            caller: Some(transaction.sender()),
            is_call: to_addr.is_some(),
            address: to_addr,
            input: Some(transaction.data.clone()),
            value: Some(transaction.value),
            gas: Some(transaction.gas),
        };

        record_runtime_call_result(
            &self.logger,
            "estimate_gas",
            self.client.estimate_gas(request),
        )
    }

    /// Checks that transaction is well formed, meets min gas price, has a valid signature,
    /// and that the contract header, if present, is valid. Returns the OasisContract (or None
    /// if no header is present), or an error message if any check fails.
    pub fn precheck_transaction(&self, raw: &Bytes) -> Fallible<CheckedTransaction> {
        // decode transaction
        let decoded: UnverifiedTransaction = rlp::decode(raw)?;

        // validate signature
        if decoded.is_unsigned() {
            return Err(format_err!("Transaction is not signed"));
        }
        let signed_transaction = SignedTransaction::new(decoded)?;

        // Check gas price.
        if signed_transaction.gas_price < self.gas_price() {
            return Err(format_err!("Insufficient gas price"));
        }

        // Validate contract deployment header (if present).
        let db = match self.get_db_snapshot() {
            Some(db) => db,
            None => {
                error!(self.logger, "Could not get db snapshot");
                return Err(format_err!("Could not parse header"));
            }
        };
        let state = match db.get_ethstate_at(BlockId::Latest) {
            Some(state) => state,
            None => {
                error!(self.logger, "Could not get state snapshot");
                return Err(format_err!("Could not parse header"));
            }
        };

        let oasis_contract = state
            .oasis_contract(&signed_transaction)
            .map_err(|err| format_err!("{}", err))?;
        Ok(CheckedTransaction {
            from_address: signed_transaction.sender(),
            contract: oasis_contract,
        })
    }

    /// Submit raw transaction to the current leader.
    ///
    /// This method returns immediately and does not wait for the transaction to
    /// be confirmed.
    pub fn send_raw_transaction(&self, raw: Bytes) -> BoxFuture<H256> {
        let checked_transaction = match self.precheck_transaction(&raw) {
            Ok(transaction) => transaction,
            Err(error) => return Box::new(future::err(error)),
        };

        let oasis_contract = checked_transaction.contract;
        let from_address = checked_transaction.from_address;
        // If we get a BlockGasLimitReached error, retry up to 5 times.
        const MAX_RETRIES: usize = 5;

        let listeners = Arc::clone(&self.listeners);
        let logger = self.logger.clone();

        Box::new(future::loop_fn(
            (MAX_RETRIES, self.client.clone(), raw, oasis_contract),
            move |(retries, client, raw, oasis_contract)| {
                let listeners = Arc::clone(&listeners);
                let logger = logger.clone();

                client
                    .execute_raw_transaction(raw.clone())
                    .and_then(move |result| {
                        // Retry on BlockGasLimitReached error.
                        if result.block_gas_limit_reached {
                            if retries == 0 {
                                RUNTIME_CALL_FAILED.inc();
                                return Err(format_err!("Block gas limit exceeded."));
                            }
                            let retries = retries - 1;
                            info!(
                                logger,
                                "execute_raw_transaction retries remaining: {}", retries
                            );
                            return Ok(future::Loop::Continue((
                                retries,
                                client,
                                raw,
                                oasis_contract,
                            )));
                        }

                        // Update contract_created metrics.
                        if result.created_contract {
                            CONTRACT_CREATED.inc();

                            let confidential =
                                oasis_contract.as_ref().map_or(false, |c| c.confidential);
                            if confidential {
                                ENC_CONTRACT_CREATED.inc();
                            }
                        }

                        match result.hash {
                            Ok(hash) => {
                                RUNTIME_CALL_SUCCEEDED.inc();
                                Client::notify_listeners(&listeners, |listener| {
                                    let output = result.output.clone();
                                    listener.notify_completed_transaction(
                                        &EthTxEntry {
                                            from_address: from_address,
                                            transaction_hash: hash,
                                        },
                                        output,
                                    );
                                });
                                Ok(future::Loop::Break(hash))
                            }
                            Err(err) => {
                                RUNTIME_CALL_FAILED.inc();
                                error!(logger, "execute_raw_transaction error"; "err" => ?err);

                                Err(format_err!("{}", err))
                            }
                        }
                    })
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethereum_types::{Address, H256};
    use test_helpers::{MockDb, MockNotificationHandler};

    #[test]
    fn test_last_hashes() {
        let db = MockDb::new();

        // get state
        let state = StateDb::new(db.cas(), db).unwrap().unwrap();

        // start with best block
        let hashes = Client::last_hashes(
            &state,
            &H256::from("c6c2b9de0cd02f617035534d69ac1413f184e5f5adf41bef9ae6271f18308778"),
        );

        assert_eq!(
            hashes[0],
            H256::from("c6c2b9de0cd02f617035534d69ac1413f184e5f5adf41bef9ae6271f18308778")
        );
        assert_eq!(
            hashes[1],
            H256::from("bacdbc2ed8161be77ed20a490e71f080017a39a1e81975e3a732da3e3d1b416b")
        );
        assert_eq!(
            hashes[2],
            H256::from("834deb56b3560fff98cbbb72dc0ea1e890cc8c32d675c80d52cab70ffbbd817f")
        );
        assert_eq!(
            hashes[3],
            H256::from("bac57123063dd9cf9a9406996a6ec6d3f5ab93cd16a05318365784477f30f8a5")
        );
        assert_eq!(
            hashes[4],
            H256::from("b1a04a31b23c3ad0dccf0c757a94463cfca1265966bc66efaf08a427e668e088")
        );
        assert_eq!(hashes[11], H256::zero());
    }

    #[test]
    fn test_envinfo() {
        let db = MockDb::new();

        // get state
        let state = StateDb::new(db.cas(), db).unwrap().unwrap();

        let envinfo = Client::get_env_info(&state);
        assert_eq!(envinfo.number, 11);
        assert_eq!(envinfo.author, Address::default());
        assert_eq!(envinfo.timestamp, 1553202944);
        assert_eq!(envinfo.difficulty, U256::zero());
        assert_eq!(
            envinfo.last_hashes[0],
            H256::from("c6c2b9de0cd02f617035534d69ac1413f184e5f5adf41bef9ae6271f18308778")
        );
    }

    #[test]
    #[cfg(feature = "pubsub")]
    fn test_headers_since() {
        let db = MockDb::new();

        // get state
        let state = StateDb::new(db.cas(), db).unwrap().unwrap();

        // blocks 1...10
        let headers = Client::headers_since(&state, 1, 10, 256);
        assert_eq!(headers.len(), 10);
        assert_eq!(
            &headers[9].hash(),
            &H256::from("c6c2b9de0cd02f617035534d69ac1413f184e5f5adf41bef9ae6271f18308778")
        );
    }

    #[test]
    #[cfg(feature = "pubsub")]
    fn test_max_block_number() {
        let client = Client::new_test_client();

        let id_1 = BlockId::Number(1);
        let id_2 = BlockId::Number(2);
        assert_eq!(client.max_block_number(id_1, id_2), id_2);

        let id_latest = BlockId::Latest;
        assert_eq!(client.max_block_number(id_latest, id_2), id_latest);

        let id_3 = BlockId::Hash(H256::from(
            "32185fcbe326513f77f85135dc5a913b1e5a645076e5ed2e34bc6ec7bc3268d4",
        ));
        assert_eq!(client.max_block_number(id_3, id_2), id_3);
    }

    #[test]
    #[cfg(feature = "pubsub")]
    fn test_min_block_number() {
        let client = Client::new_test_client();

        let id_1 = BlockId::Number(1);
        let id_2 = BlockId::Number(2);
        assert_eq!(client.min_block_number(id_1, id_2), id_1);

        let id_earliest = BlockId::Earliest;
        assert_eq!(client.min_block_number(id_earliest, id_2), id_earliest);

        let id_3 = BlockId::Hash(H256::from(
            "75be890ab64005e4239cfc257349c536fdde555a211c663b9235abb2ec21e56e",
        ));
        assert_eq!(client.min_block_number(id_3, id_2), id_2);
    }

    #[test]
    #[cfg(feature = "pubsub")]
    fn test_pubsub_notify() {
        let client = Client::new_test_client();

        let handler = Arc::new(MockNotificationHandler::new());
        client.add_listener(Arc::downgrade(&handler) as Weak<_>);

        let headers = handler.get_notified_headers();
        let log_notifications = handler.get_log_notifications();
        assert_eq!(headers.len(), 0);
        assert_eq!(log_notifications.len(), 0);

        client.new_blocks();

        let headers = handler.get_notified_headers();
        assert_eq!(headers.len(), 10);
        assert_eq!(
            headers[9].hash(),
            H256::from("c6c2b9de0cd02f617035534d69ac1413f184e5f5adf41bef9ae6271f18308778")
        );

        let log_notifications = handler.get_log_notifications();
        assert_eq!(log_notifications.len(), 1);
        assert_eq!(log_notifications[0].0, BlockId::Number(1));
        assert_eq!(log_notifications[0].1, BlockId::Number(10));
    }
}
