use std::sync::Arc;

use bytes::Bytes;
use common_types::log_entry::LocalizedLogEntry;
use ethcore::blockchain::{BlockProvider, TransactionAddress};
use ethcore::client::{BlockId, EnvInfo, LastHashes, StateOrBlock, TransactionId};
use ethcore::encoded;
use ethcore::engines::EthEngine;
use ethcore::error::CallError;
use ethcore::executive::{contract_address, Executed, Executive, TransactOptions};
use ethcore::filter::Filter as EthcoreFilter;
use ethcore::header::BlockNumber;
use ethcore::receipt::LocalizedReceipt;
use ethcore::spec::Spec;
use ethereum_types::{Address, H256, U256};
use futures::future::Future;
use runtime_ethereum;
use transaction::{Action, LocalizedTransaction, SignedTransaction};

use client_utils;
use ekiden_core::error::Error;
use ethereum_api::{Filter, Log, Receipt, Transaction, TransactionRequest};

use state::{self, EthState, StateDb};
use util::from_block_id;

// record contract call outcome
fn contract_call_result<T>(call: &str, result: Result<T, Error>, default: T) -> T {
    match result {
        Ok(val) => {
            measure_counter_inc!("contract_call_succeeded");
            val
        }
        Err(e) => {
            measure_counter_inc!("contract_call_failed");
            error!("{}: {:?}", call, e);
            default
        }
    }
}

pub struct Client {
    client: runtime_ethereum::Client,
    engine: Arc<EthEngine>,
    snapshot_manager: client_utils::db::Manager,
    eip86_transition: u64,
}

impl Client {
    pub fn new(
        spec: &Spec,
        snapshot_manager: client_utils::db::Manager,
        client: runtime_ethereum::Client,
    ) -> Self {
        Self {
            client: client,
            engine: spec.engine.clone(),
            snapshot_manager: snapshot_manager,
            eip86_transition: spec.params().eip86_transition,
        }
    }

    pub fn eip86_transition(&self) -> u64 {
        self.eip86_transition
    }

    #[cfg(feature = "caching")]
    fn get_db_snapshot(&self) -> Option<StateDb> {
        state::StateDb::new(self.snapshot_manager.get_snapshot())
    }

    // block-related
    pub fn best_block_number(&self) -> BlockNumber {
        #[cfg(feature = "caching")]
        {
            if let Some(snapshot) = self.get_db_snapshot() {
                return snapshot.best_block_number();
            }
        }
        contract_call_result(
            "get_block_height",
            self.client.get_block_height(false).wait(),
            U256::from(0),
        ).into()
    }

    pub fn block(&self, id: BlockId) -> Option<encoded::Block> {
        #[cfg(feature = "caching")]
        {
            if let Some(snapshot) = self.get_db_snapshot() {
                return self.block_hash(id).and_then(|h| snapshot.block(&h));
            }
        }
        contract_call_result::<Option<Vec<u8>>>(
            "get_block",
            self.client.get_block(from_block_id(id)).wait(),
            None,
        ).map(|block| encoded::Block::new(block))
    }

    #[cfg(feature = "caching")]
    pub fn block_hash(&self, id: BlockId) -> Option<H256> {
        if let BlockId::Hash(hash) = id {
            Some(hash)
        } else {
            if let Some(snapshot) = self.get_db_snapshot() {
                match id {
                    BlockId::Hash(_hash) => unreachable!(),
                    BlockId::Number(number) => snapshot.block_hash(number),
                    BlockId::Earliest => snapshot.block_hash(0),
                    BlockId::Latest => snapshot.best_block_hash(),
                }
            } else {
                None
            }
        }
    }

    #[cfg(not(feature = "caching"))]
    pub fn block_hash(&self, id: BlockId) -> Option<H256> {
        if let BlockId::Hash(hash) = id {
            Some(hash)
        } else {
            contract_call_result(
                "get_block_hash",
                self.client.get_block_hash(from_block_id(id)).wait(),
                None,
            )
        }
    }

    // transaction-related
    #[cfg(feature = "caching")]
    pub fn transaction(&self, id: TransactionId) -> Option<LocalizedTransaction> {
        if let Some(snapshot) = self.get_db_snapshot() {
            let address = match id {
                TransactionId::Hash(ref hash) => snapshot.transaction_address(hash),
                TransactionId::Location(id, index) => {
                    Self::id_to_block_hash(&snapshot, id).map(|hash| TransactionAddress {
                        block_hash: hash,
                        index: index,
                    })
                }
            };
            address.and_then(|addr| snapshot.transaction(&addr))
        } else {
            None
        }
    }

    #[cfg(not(feature = "caching"))]
    pub fn transaction(&self, hash: H256) -> Option<Transaction> {
        contract_call_result(
            "get_transaction",
            self.client.get_transaction(hash).wait(),
            None,
        )
    }

    #[cfg(feature = "caching")]
    pub fn transaction_receipt(&self, hash: H256) -> Option<LocalizedReceipt> {
        if let Some(snapshot) = self.get_db_snapshot() {
            let address = snapshot.transaction_address(&hash)?;
            let receipt = snapshot.transaction_receipt(&address)?;
            let mut tx = snapshot.transaction(&address)?;

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

    #[cfg(not(feature = "caching"))]
    pub fn transaction_receipt(&self, hash: H256) -> Option<Receipt> {
        contract_call_result("get_receipt", self.client.get_receipt(hash).wait(), None)
    }

    fn id_to_block_hash(snapshot: &StateDb, id: BlockId) -> Option<H256> {
        match id {
            BlockId::Hash(hash) => Some(hash),
            BlockId::Number(number) => snapshot.block_hash(number),
            BlockId::Earliest => snapshot.block_hash(0),
            BlockId::Latest => snapshot.best_block_hash(),
        }
    }

    #[cfg(feature = "caching")]
    pub fn logs(&self, filter: EthcoreFilter) -> Vec<LocalizedLogEntry> {
        if let Some(snapshot) = self.get_db_snapshot() {
            let fetch_logs = || {
                let from_hash = Self::id_to_block_hash(&snapshot, filter.from_block)?;
                let from_number = snapshot.block_number(&from_hash)?;
                // NOTE: there appears to be a bug in parity with to_hash:
                // https://github.com/ekiden/parity/blob/master/ethcore/src/client/client.rs#L1856
                let to_hash = Self::id_to_block_hash(&snapshot, filter.to_block)?;

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
                        let header = snapshot.block_header_data(&current_hash)?;
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

                Some(snapshot.logs(blocks, |entry| filter.matches(entry), filter.limit))
            };

            fetch_logs().unwrap_or_default()
        } else {
            vec![]
        }
    }

    #[cfg(not(feature = "caching"))]
    pub fn logs(&self, filter: EthcoreFilter) -> Vec<Log> {
        let filter = Filter {
            from_block: from_block_id(filter.from_block),
            to_block: from_block_id(filter.to_block),
            address: match filter.address {
                Some(address) => Some(address.into_iter().map(Into::into).collect()),
                None => None,
            },
            topics: filter.topics.into_iter().map(Into::into).collect(),
            limit: filter.limit.map(Into::into),
        };
        contract_call_result("get_logs", self.client.get_logs(filter).wait(), vec![])
    }

    // account state-related
    #[cfg(feature = "caching")]
    fn get_ethstate_snapshot(&self) -> Option<EthState> {
        self.get_db_snapshot()?.get_ethstate()
    }

    pub fn balance(&self, address: &Address, state: StateOrBlock) -> Option<U256> {
        #[cfg(feature = "caching")]
        {
            if let Some(snapshot) = self.get_ethstate_snapshot() {
                match snapshot.balance(&address) {
                    Ok(balance) => return Some(balance),
                    Err(_) => return None,
                }
            }
        }
        contract_call_result(
            "get_account_balance",
            self.client.get_account_balance(*address).wait().map(Some),
            None,
        )
    }

    pub fn code(&self, address: &Address, state: StateOrBlock) -> Option<Option<Bytes>> {
        // TODO: differentiate between no account vs no code?
        #[cfg(feature = "caching")]
        {
            if let Some(snapshot) = self.get_ethstate_snapshot() {
                match snapshot.code(&address) {
                    Ok(code) => return Some(code.map(|c| (&*c).clone())),
                    Err(_) => return None,
                }
            }
        }
        contract_call_result(
            "get_account_code",
            self.client.get_account_code(*address).wait().map(Some),
            None,
        )
    }

    pub fn nonce(&self, address: &Address, id: BlockId) -> Option<U256> {
        #[cfg(feature = "caching")]
        {
            if let Some(snapshot) = self.get_ethstate_snapshot() {
                match snapshot.nonce(&address) {
                    Ok(nonce) => return Some(nonce),
                    Err(_) => return None,
                }
            }
        }
        contract_call_result(
            "get_account_nonce",
            self.client.get_account_nonce(*address).wait().map(Some),
            None,
        )
    }

    pub fn storage_at(
        &self,
        address: &Address,
        position: &H256,
        state: StateOrBlock,
    ) -> Option<H256> {
        #[cfg(feature = "caching")]
        {
            if let Some(snapshot) = self.get_ethstate_snapshot() {
                match snapshot.storage_at(address, position) {
                    Ok(val) => return Some(val),
                    Err(_) => return None,
                }
            }
        }
        contract_call_result(
            "get_storage_at",
            self.client
                .get_storage_at((*address, *position))
                .wait()
                .map(Some),
            None,
        )
    }

    fn last_hashes(snapshot: &StateDb, parent_hash: &H256) -> Arc<LastHashes> {
        let mut last_hashes = LastHashes::new();
        last_hashes.resize(256, H256::default());
        last_hashes[0] = parent_hash.clone();
        for i in 0..255 {
            match snapshot.block_details(&last_hashes[i]) {
                Some(details) => {
                    last_hashes[i + 1] = details.parent.clone();
                }
                None => break,
            }
        }
        Arc::new(last_hashes)
    }

    fn get_env_info(snapshot: &StateDb) -> EnvInfo {
        let header = snapshot
            .best_block_hash()
            .and_then(|hash| snapshot.block_header_data(&hash))
            .expect("No best block");
        EnvInfo {
            number: header.number(),
            author: header.author().clone(),
            timestamp: header.timestamp(),
            difficulty: header.difficulty().clone(),
            last_hashes: Self::last_hashes(snapshot, &header.parent_hash()),
            gas_used: U256::default(),
            gas_limit: U256::max_value(),
        }
    }

    // transaction-related
    #[cfg(feature = "caching")]
    pub fn call(&self, transaction: &SignedTransaction) -> Result<Executed, CallError> {
        let db = match self.get_db_snapshot() {
            Some(snapshot) => snapshot,
            None => {
                error!("Could not get state snapshot");
                return Err(CallError::StateCorrupt);
            }
        };
        let mut state = match db.get_ethstate() {
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
        let ret =
            Executive::new(&mut state, &env_info, machine).transact_virtual(transaction, options)?;
        Ok(ret)
    }

    #[cfg(not(feature = "caching"))]
    pub fn call(&self, request: TransactionRequest) -> Result<Bytes, String> {
        contract_call_result(
            "simulate_transaction",
            self.client
                .simulate_transaction(request)
                .wait()
                .map(|r| r.result),
            Err("no response from runtime".to_string()),
        )
    }

    #[cfg(feature = "caching")]
    pub fn estimate_gas(&self, transaction: &SignedTransaction) -> Result<U256, CallError> {
        let db = match self.get_db_snapshot() {
            Some(snapshot) => snapshot,
            None => {
                error!("Could not get state snapshot");
                return Err(CallError::StateCorrupt);
            }
        };
        let mut state = match db.get_ethstate() {
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
        let ret =
            Executive::new(&mut state, &env_info, machine).transact_virtual(transaction, options)?;
        Ok(ret.gas_used)
    }

    #[cfg(not(feature = "caching"))]
    pub fn estimate_gas(&self, request: TransactionRequest) -> Result<U256, String> {
        contract_call_result(
            "simulate_transaction",
            self.client
                .simulate_transaction(request)
                .wait()
                .map(|r| Ok(r.used_gas)),
            Err("no response from runtime".to_string()),
        )
    }

    pub fn send_raw_transaction(&self, raw: Bytes) -> Result<H256, String> {
        contract_call_result(
            "execute_raw_transaction",
            self.client
                .execute_raw_transaction(raw)
                .wait()
                .map(|r| r.hash),
            Err("no response from runtime".to_string()),
        )
    }
}
