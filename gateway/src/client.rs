use std::marker::{Send, Sync};
use std::sync::{Arc, Mutex, RwLock, Weak};

use bytes::Bytes;
use common_types::log_entry::LocalizedLogEntry;
use ethcore::blockchain::{BlockProvider, TransactionAddress};
use ethcore::encoded;
use ethcore::engines::EthEngine;
use ethcore::error::CallError;
use ethcore::executive::{contract_address, Executed, Executive, TransactOptions};
use ethcore::filter::Filter as EthcoreFilter;
use ethcore::header::BlockNumber;
use ethcore::ids::{BlockId, TransactionId};
use ethcore::receipt::LocalizedReceipt;
use ethcore::rlp;
use ethcore::spec::Spec;
use ethcore::transaction::UnverifiedTransaction;
use ethcore::vm::{EnvInfo, LastHashes};
use ethereum_types::{Address, H256, U256};
use futures::future::Future;
#[cfg(test)]
use grpcio;
use hash::keccak;
use parity_rpc::v1::types::Bytes as RpcBytes;
use runtime_ethereum;
use std::time::{SystemTime, UNIX_EPOCH};
use traits::confidential::PublicKeyResult;
use transaction::{Action, LocalizedTransaction, SignedTransaction};

use client_utils;
use client_utils::db::Snapshot;
use ekiden_common::bytes::B512;
use ekiden_common::environment::Environment;
use ekiden_core::error::Error;
use ekiden_core::futures::prelude::*;
use ekiden_db_trusted::Database;
use ekiden_keymanager_client::KeyManager as EkidenKeyManager;
use ekiden_keymanager_common::ContractId;
use ekiden_storage_base::StorageBackend;
#[cfg(test)]
use ekiden_storage_dummy::DummyStorageBackend;
use ethereum_api::TransactionRequest;
use state::{self, StateDb};
use storage::Web3GlobalStorage;
#[cfg(test)]
use test_helpers::{self, MockDb};
#[cfg(test)]
use util;
use util::from_block_id;

/// Record runtime call outcome.
fn record_runtime_call_result<F, T>(call: &'static str, result: F) -> BoxFuture<T>
where
    T: 'static + Send,
    F: 'static + Future<Item = T, Error = Error> + Send,
{
    result
        .then(move |result| {
            match result {
                Ok(_) => {
                    measure_counter_inc!("runtime_call_succeeded");
                }
                Err(ref error) => {
                    measure_counter_inc!("runtime_call_failed");
                    error!("{}: {:?}", call, error);
                }
            }

            result
        })
        .into_box()
}

/// An actor listening to chain events.
pub trait ChainNotify: Send + Sync {
    fn has_heads_subscribers(&self) -> bool;

    /// Notifies about new headers.
    fn notify_heads(&self, headers: &[encoded::Header]);

    /// Notifies about new log filter matches.
    fn notify_logs(&self, from_block: BlockId, to_block: BlockId);
}

pub struct Client {
    client: runtime_ethereum::Client,
    engine: Arc<EthEngine>,
    snapshot_manager: Option<client_utils::db::Manager>,
    eip86_transition: u64,
    environment: Arc<Environment>,
    storage_backend: Arc<StorageBackend>,
    storage: Arc<RwLock<Web3GlobalStorage>>,
    /// The most recent block for which we have sent notifications.
    notified_block_number: Mutex<BlockNumber>,
    listeners: RwLock<Vec<Weak<ChainNotify>>>,
    gas_price: U256,
}

impl Client {
    pub fn new(
        spec: &Spec,
        snapshot_manager: Option<client_utils::db::Manager>,
        client: runtime_ethereum::Client,
        environment: Arc<Environment>,
        backend: Arc<StorageBackend>,
        gas_price: U256,
    ) -> Self {
        let storage = Web3GlobalStorage::new(backend.clone());

        // get current block number from db snapshot (or 0)
        let current_block_number = match snapshot_manager {
            Some(ref manager) => match state::StateDb::new(backend.clone(), manager.get_snapshot())
            {
                Ok(db) => db.map_or(0, |db| db.best_block_number()),
                Err(_) => 0,
            },
            None => 0,
        };

        Self {
            client: client,
            engine: spec.engine.clone(),
            snapshot_manager: snapshot_manager,
            eip86_transition: spec.params().eip86_transition,
            environment,
            storage_backend: backend,
            storage: Arc::new(RwLock::new(storage)),
            // start at current block
            notified_block_number: Mutex::new(current_block_number),
            listeners: RwLock::new(vec![]),
            gas_price: gas_price,
        }
    }

    /// A blockchain client for unit tests.
    #[cfg(test)]
    pub fn get_test_client() -> Self {
        let spec = &util::load_spec();
        let grpc_environment = grpcio::EnvBuilder::new().build();
        let environment = Arc::new(ekiden_common::environment::GrpcEnvironment::new(
            grpc_environment,
        ));
        let storage_backend = Arc::new(DummyStorageBackend::new());
        let storage = Web3GlobalStorage::new(storage_backend.clone());
        Self {
            client: test_helpers::get_test_runtime_client(),
            engine: spec.engine.clone(),
            snapshot_manager: None,
            eip86_transition: spec.params().eip86_transition,
            environment: environment,
            storage_backend,
            storage: Arc::new(RwLock::new(storage)),
            notified_block_number: Mutex::new(0),
            listeners: RwLock::new(vec![]),
            gas_price: U256::from(1_000_000_000),
        }
    }

    /// Spawn a future in our environment and wait for its result.
    pub fn block_on<F, R, E>(&self, future: F) -> Result<R, E>
    where
        F: Send + 'static + Future<Item = R, Error = E>,
        R: Send + 'static,
        E: Send + 'static,
    {
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        self.environment.spawn(Box::new(future.then(move |result| {
            drop(result_tx.send(result));
            Ok(())
        })));
        result_rx
            .recv()
            .expect("block_on: Environment dropped our result sender")
    }

    /// Notify listeners of new blocks.
    #[cfg(feature = "pubsub")]
    pub fn new_blocks(&self) {
        const MAX_HEADERS: u64 = 256;

        let mut last_block = self.notified_block_number.lock().unwrap();

        measure_histogram_timer!("pubsub_notify_time");

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
        for listener in &*self.listeners.read().unwrap() {
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
    fn get_db_snapshot(&self) -> Option<StateDb<Snapshot>> {
        match self.snapshot_manager {
            Some(ref manager) => {
                match state::StateDb::new(self.storage_backend.clone(), manager.get_snapshot()) {
                    Ok(db) => db,
                    Err(e) => {
                        measure_counter_inc!("read_state_failed");
                        error!("Could not get db snapshot: {:?}", e);
                        None
                    }
                }
            }
            None => None,
        }
    }

    /// Returns a MockDb-backed StateDb for unit tests.
    #[cfg(test)]
    fn get_db_snapshot(&self) -> Option<StateDb<MockDb>> {
        let mut db = MockDb::new();
        db.populate();
        StateDb::new(db.storage(), db).unwrap()
    }

    // block-related

    pub fn best_block_number(&self) -> BlockNumber {
        if let Some(db) = self.get_db_snapshot() {
            return db.best_block_number();
        }

        // Fall back to runtime call if database has not been initialized.
        // NOTE: We need to block on this call as making this method futures-aware
        //       would complicate the consumers a lot.
        self.block_on(record_runtime_call_result(
            "get_block_height",
            self.client
                .get_block_height(false)
                .map(|height| height.into()),
        )).unwrap_or_default()
    }

    pub fn block(&self, id: BlockId) -> BoxFuture<Option<encoded::Block>> {
        if let Some(db) = self.get_db_snapshot() {
            return future::ok(self.block_hash(id).and_then(|h| db.block(&h))).into_box();
        }

        // Fall back to runtime call if database has not been initialized.
        record_runtime_call_result(
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

            Some(LocalizedReceipt {
                transaction_hash: transaction_hash,
                transaction_index: transaction_index,
                block_hash: block_hash,
                block_number: block_number,
                cumulative_gas_used: receipt.gas_used,
                gas_used: receipt.gas_used,
                contract_address: match tx.action {
                    Action::Call(_) => None,
                    Action::Create => Some(
                        contract_address(
                            self.engine.create_address_scheme(block_number),
                            &tx.sender(),
                            &tx.nonce,
                            &tx.data,
                        ).0,
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
        T: 'static + Database + Send + Sync,
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
                    measure_counter_inc!("log_filter_rejected");
                    error!(
                        "getLogs rejected block range: ({:?}, {:?})",
                        from_number, to_number
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
                        Ok(balance) => future::ok(balance).into_box(),
                        Err(e) => {
                            measure_counter_inc!("read_state_failed");
                            error!("Could not get balance from ethstate: {:?}", e);
                            future::err(Error::new("Could not get balance")).into_box()
                        }
                    }
                } else {
                    future::err(Error::new("Unknown block")).into_box()
                }
            }
            None => {
                // Fall back to runtime call if database has not been initialized.
                record_runtime_call_result(
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
                        Ok(code) => future::ok(code.map(|c| (&*c).clone())).into_box(),
                        Err(e) => {
                            measure_counter_inc!("read_state_failed");
                            error!("Could not get code from ethstate: {:?}", e);
                            future::err(Error::new("Could not get code")).into_box()
                        }
                    }
                } else {
                    future::err(Error::new("Unknown block")).into_box()
                }
            }
            None => {
                // Fall back to runtime call if database has not been initialized.
                record_runtime_call_result(
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
                        Ok(nonce) => future::ok(nonce).into_box(),
                        Err(e) => {
                            measure_counter_inc!("read_state_failed");
                            error!("Could not get nonce from ethstate: {:?}", e);
                            future::err(Error::new("Could not get nonce")).into_box()
                        }
                    }
                } else {
                    future::err(Error::new("Unknown block")).into_box()
                }
            }
            None => {
                // Fall back to runtime call if database has not been initialized.
                record_runtime_call_result(
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
                        Ok(val) => future::ok(val).into_box(),
                        Err(e) => {
                            measure_counter_inc!("read_state_failed");
                            error!("Could not get storage from ethstate: {:?}", e);
                            future::err(Error::new("Could not get storage")).into_box()
                        }
                    }
                } else {
                    future::err(Error::new("Unknown block")).into_box()
                }
            }
            None => {
                // Fall back to runtime call if database has not been initialized.
                record_runtime_call_result(
                    "get_storage_at",
                    self.client.get_storage_at((*address, *position)),
                )
            }
        }
    }

    fn last_hashes<T>(db: &StateDb<T>, parent_hash: &H256) -> Arc<LastHashes>
    where
        T: 'static + Database + Send + Sync,
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
        T: 'static + Database + Send + Sync,
    {
        // limit to `max` headers
        let start = if end - start + 1 >= max {
            end - max + 1
        } else {
            start
        };

        let mut head = db.block_hash(end)
            .and_then(|hash| db.block_header_data(&hash))
            .expect("Invalid block number");

        let mut headers = Vec::with_capacity((end - start + 1) as usize);

        loop {
            headers.push(head.clone());
            if head.number() <= start {
                break;
            }
            head = db.block_header_data(&head.parent_hash())
                .expect("Chain is corrupt");
        }
        headers.reverse();
        headers
    }

    fn get_env_info<T>(db: &StateDb<T>) -> EnvInfo
    where
        T: 'static + Database + Send + Sync,
    {
        let parent = db.best_block_hash()
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
                error!("Could not get db snapshot");
                return Err(CallError::StateCorrupt);
            }
        };
        let mut state = match db.get_ethstate_at(id) {
            Some(state) => state,
            None => {
                error!("Could not get state snapshot");
                return Err(CallError::StateCorrupt);
            }
        };

        let env_info = Self::get_env_info(&db);
        let machine = self.engine.machine();
        let options = TransactOptions::with_no_tracing()
            .dont_check_nonce()
            .save_output_from_contract();
        let ret = Executive::new(
            &mut state,
            &env_info,
            machine,
            &*self.storage.read().unwrap(),
        ).transact_virtual(transaction, options)?;
        Ok(ret)
    }

    pub fn call_enc(&self, request: TransactionRequest, _id: BlockId) -> BoxFuture<Bytes> {
        record_runtime_call_result(
            "simulate_transaction",
            self.client
                .simulate_transaction(request)
                .and_then(|r| r.result.map_err(|error| Error::new(error))),
        )
    }

    pub fn estimate_gas(
        &self,
        transaction: &SignedTransaction,
        id: BlockId,
    ) -> Result<U256, CallError> {
        let db = match self.get_db_snapshot() {
            Some(db) => db,
            None => {
                error!("Could not get db snapshot");
                return Err(CallError::StateCorrupt);
            }
        };
        let mut state = match db.get_ethstate_at(id) {
            Some(state) => state,
            None => {
                error!("Could not get state snapshot");
                return Err(CallError::StateCorrupt);
            }
        };

        let env_info = Self::get_env_info(&db);
        let machine = self.engine.machine();
        let options = TransactOptions::with_no_tracing()
            .dont_check_nonce()
            .save_output_from_contract();
        let ret = Executive::new(
            &mut state,
            &env_info,
            machine,
            &*self.storage.read().unwrap(),
        ).transact_virtual(transaction, options)?;
        Ok(ret.gas_used + ret.refunded)
    }

    /// Checks whether transaction is well formed and meets min gas price.
    pub fn precheck_transaction(&self, raw: &Bytes) -> Result<(), String> {
        let decoded: UnverifiedTransaction = match rlp::decode(raw) {
            Ok(t) => t,
            Err(e) => return Err(e.to_string()),
        };
        let unsigned = decoded.as_unsigned();
        if unsigned.gas_price < self.gas_price() {
            return Err("Insufficient gas price".to_string());
        }

        Ok(())
    }

    /// Submit raw transaction to the current leader.
    ///
    /// This method returns immediately and does not wait for the transaction to
    /// be confirmed.
    pub fn send_raw_transaction(&self, raw: Bytes) -> BoxFuture<H256> {
        if let Err(error) = self.precheck_transaction(&raw) {
            return future::err(Error::new(error)).into_box();
        };

        record_runtime_call_result(
            "execute_raw_transaction",
            self.client.execute_raw_transaction(raw).and_then(|result| {
                if result.created_contract {
                    measure_counter_inc!("contract_created");
                }

                result.hash.map_err(|error| Error::new(error))
            }),
        )
    }

    /// Returns the public key for the given contract from the key manager.
    pub fn public_key(&self, contract: Address) -> Result<PublicKeyResult, String> {
        let contract_id: ContractId =
            ekiden_core::bytes::H256::from(&keccak(contract.to_vec())[..]);

        let public_key = EkidenKeyManager::instance()
            .expect("Should always have an key manager client")
            .get_public_key(contract_id)
            .map_err(|err| err.description().to_string())?;

        // TODO: V1 should be issued by the key manager
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(PublicKeyResult {
            public_key: RpcBytes::from(public_key.to_vec()),
            timestamp: timestamp,
            signature: RpcBytes::from(B512::from(2).to_vec()), // todo: v1
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethereum_types::{Address, H256};

    use test_helpers::{MockDb, MockNotificationHandler};

    #[test]
    fn test_last_hashes() {
        let mut db = MockDb::new();
        // populate the db with test data
        db.populate();

        // get state
        let state = StateDb::new(db.storage(), db).unwrap().unwrap();

        // start with best block
        let hashes = Client::last_hashes(
            &state,
            &H256::from("832e166d73a1baddb00d65de04086616548e3c96b0aaf0f9fe1939e29868c118"),
        );

        assert_eq!(
            hashes[0],
            H256::from("832e166d73a1baddb00d65de04086616548e3c96b0aaf0f9fe1939e29868c118")
        );
        assert_eq!(
            hashes[1],
            H256::from("75be890ab64005e4239cfc257349c536fdde555a211c663b9235abb2ec21e56e")
        );
        assert_eq!(
            hashes[2],
            H256::from("613afac8fd33fd7a35b8928e68f6abc031ca8e16c35caa2eaa7518c4e753cffc")
        );
        assert_eq!(
            hashes[3],
            H256::from("9a4ffe2733a837c80d0b7e2fd63b838806e3b8294dab3ad86249619b28fd9526")
        );
        assert_eq!(
            hashes[4],
            H256::from("3546adf1c89e32acd11093f6f78468f5db413a207843aded872397821ea685ae")
        );
        assert_eq!(hashes[5], H256::zero());
    }

    #[test]
    fn test_envinfo() {
        let mut db = MockDb::new();
        // populate the db with test data
        db.populate();

        // get state
        let state = StateDb::new(db.storage(), db).unwrap().unwrap();

        let envinfo = Client::get_env_info(&state);
        assert_eq!(envinfo.number, 5);
        assert_eq!(envinfo.author, Address::default());
        assert_eq!(envinfo.timestamp, 1539086487);
        assert_eq!(envinfo.difficulty, U256::zero());
        assert_eq!(
            envinfo.last_hashes[0],
            H256::from("832e166d73a1baddb00d65de04086616548e3c96b0aaf0f9fe1939e29868c118")
        );
    }

    #[test]
    #[cfg(feature = "pubsub")]
    fn test_headers_since() {
        let mut db = MockDb::new();
        // populate the db with test data
        db.populate();

        // get state
        let state = StateDb::new(db.storage(), db).unwrap().unwrap();

        // blocks 1...4
        let headers = Client::headers_since(&state, 1, 4, 256);
        assert_eq!(headers.len(), 4);
        assert_eq!(
            &headers[3].hash(),
            &H256::from("832e166d73a1baddb00d65de04086616548e3c96b0aaf0f9fe1939e29868c118")
        );
    }

    #[test]
    #[cfg(feature = "pubsub")]
    fn test_max_block_number() {
        let client = Client::get_test_client();

        let id_1 = BlockId::Number(1);
        let id_2 = BlockId::Number(2);
        assert_eq!(client.max_block_number(id_1, id_2), id_2);

        let id_latest = BlockId::Latest;
        assert_eq!(client.max_block_number(id_latest, id_2), id_latest);

        let id_3 = BlockId::Hash(H256::from(
            "75be890ab64005e4239cfc257349c536fdde555a211c663b9235abb2ec21e56e",
        ));
        assert_eq!(client.max_block_number(id_3, id_2), id_3);
    }

    #[test]
    #[cfg(feature = "pubsub")]
    fn test_min_block_number() {
        let client = Client::get_test_client();

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
        let client = Client::get_test_client();

        let handler = Arc::new(MockNotificationHandler::new());
        client.add_listener(Arc::downgrade(&handler) as Weak<_>);

        let headers = handler.get_notified_headers();
        let log_notifications = handler.get_log_notifications();
        assert_eq!(headers.len(), 0);
        assert_eq!(log_notifications.len(), 0);

        client.new_blocks();

        let headers = handler.get_notified_headers();
        assert_eq!(headers.len(), 4);
        assert_eq!(
            headers[3].hash(),
            H256::from("832e166d73a1baddb00d65de04086616548e3c96b0aaf0f9fe1939e29868c118")
        );

        let log_notifications = handler.get_log_notifications();
        assert_eq!(log_notifications.len(), 1);
        assert_eq!(log_notifications[0].0, BlockId::Number(1));
        assert_eq!(log_notifications[0].1, BlockId::Number(4));
    }
}
