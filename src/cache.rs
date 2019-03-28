//! Parity blockchain cache.
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use ekiden_keymanager_client::KeyManagerClient;
use ekiden_runtime::{
    common::{crypto::hash::Hash, logger::get_logger},
    storage::{StorageContext, CAS, MKVS},
};
use ethcore::{
    self,
    block::{IsBlock, LockedBlock, OpenBlock},
    blockchain::{BlockChain, BlockProvider, ExtrasInsert},
    encoded::Block,
    engines::ForkChoice,
    filter::Filter as EthcoreFilter,
    header::Header,
    kvdb::{self, KeyValueDB},
    rlp,
    state::backend::Wrapped as WrappedBackend,
    transaction::Action,
    types::{ids::BlockId, receipt::TransactionOutcome, BlockNumber},
};
use ethereum_types::{Address, H256, U256};
use failure::{format_err, Fail, Fallible};
use io_context::Context as IoContext;
use runtime_ethereum_api::{Filter, Log, Receipt, Transaction};
use runtime_ethereum_common::{
    confidential::ConfidentialCtx, get_factories, BlockchainStateDb, State, StorageHashDB,
};
use slog::{debug, info, Logger};

use crate::{genesis, util};

/// Cache error.
#[derive(Debug, Fail)]
pub enum CacheError {
    #[fail(display = "cache is not initialized")]
    NotInitialized,
}

struct Inner {
    /// Last known Ekiden state root.
    last_root_hash: Hash,
    /// Actual cache.
    chain: Option<BlockChain>,
}

/// Parity blockchain cache.
pub struct Cache {
    inner: Mutex<Inner>,
    key_manager: Arc<KeyManagerClient>,
    state_db: Arc<BlockchainStateDb<ThreadLocalMKVS>>,
    hash_db: StorageHashDB<ThreadLocalMKVS>,
    logger: Logger,
}

impl Cache {
    /// Create a new uninitialized blockchain cache.
    pub fn new(key_manager: Arc<KeyManagerClient>) -> Self {
        let state_db = Arc::new(BlockchainStateDb::new(ThreadLocalMKVS));

        Self {
            inner: Mutex::new(Inner {
                last_root_hash: Hash::default(),
                chain: None,
            }),
            key_manager,
            hash_db: StorageHashDB::new(Arc::new(ThreadLocalCAS), state_db.clone()),
            state_db,
            logger: get_logger("ethereum/cache"),
        }
    }

    /// Initialize (or re-initialize) the blockchain cache.
    pub fn init(&self, root_hash: Hash) -> Fallible<()> {
        let mut inner = self.inner.lock().unwrap();

        // Check if no re-initialization is needed by comparing root hashes.
        if inner.chain.is_some() {
            if inner.last_root_hash == root_hash {
                // Last root hash matches, no need to re-initialize.
                return Ok(());
            }

            info!(self.logger, "State root hash has changed, cache is invalid";
                "last_root" => ?inner.last_root_hash,
                "current_root" => ?root_hash
            );
        }

        info!(self.logger, "Initializing blockchain cache");

        // Initialize Ethereum state with the genesis block in case there is none.
        let backend = WrappedBackend(Box::new(self.hash_db.clone()));
        genesis::SPEC
            .ensure_db_good(backend, &get_factories())
            .map_err(|err| format_err!("{}", err))?;
        self.hash_db.commit();

        info!(self.logger, "Genesis state initialized");

        inner.chain = Some(BlockChain::new(
            // Default configuration,
            Default::default(),
            // Genesis block.
            &*genesis::SPEC.genesis_block(),
            // Blockchain database.
            self.state_db.clone(),
        ));

        info!(self.logger, "Blockchain cache initialized");

        Ok(())
    }

    /// Set the finalized state root hash.
    pub fn finalize_root(&self, root_hash: Hash) {
        let mut inner = self.inner.lock().unwrap();
        inner.last_root_hash = root_hash;
    }

    pub fn get_state(&self, ctx: Arc<IoContext>) -> Fallible<State> {
        let inner = self.inner.lock().unwrap();
        let chain = inner.chain.as_ref().ok_or(CacheError::NotInitialized)?;

        let root = chain.best_block_header().state_root().clone();
        Ok(State::from_existing(
            WrappedBackend(Box::new(self.hash_db.clone())),
            root,
            U256::zero(), /* account_start_nonce */
            get_factories(),
            Some(Box::new(ConfidentialCtx::new(
                ctx,
                self.key_manager.clone(),
            ))),
        )?)
    }

    pub fn new_block(&self, ctx: Arc<IoContext>) -> Fallible<OpenBlock<'static>> {
        let inner = self.inner.lock().unwrap();
        let chain = inner.chain.as_ref().ok_or(CacheError::NotInitialized)?;

        debug!(self.logger, "Opening new block");

        let backend = WrappedBackend(Box::new(self.hash_db.clone()));
        let parent = chain.best_block_header();

        debug!(self.logger, "Determined block parent"; "parent" => ?parent);

        Ok(OpenBlock::new(
            &*genesis::SPEC.engine,
            get_factories(),
            cfg!(debug_assertions),                         /* tracing */
            backend,                                        /* state_db */
            &parent,                                        /* parent */
            self.last_hashes_chain(&chain, &parent.hash()), /* last hashes */
            Address::default(),                             /* author */
            *genesis::GAS_LIMIT,                            /* block gas limit */
            vec![],                                         /* extra data */
            true,                                           /* is epoch_begin */
            &mut Vec::new().into_iter(),                    /* ancestry */
            Some(Box::new(ConfidentialCtx::new(
                ctx,
                self.key_manager.clone(),
            ))),
        )
        .map_err(|err| format_err!("Failed to open block: {}", err))?)
    }

    pub fn last_hashes(&self, parent_hash: &H256) -> Fallible<Arc<Vec<H256>>> {
        let inner = self.inner.lock().unwrap();
        let chain = inner.chain.as_ref().ok_or(CacheError::NotInitialized)?;

        Ok(self.last_hashes_chain(chain, parent_hash))
    }

    fn last_hashes_chain(&self, chain: &BlockChain, parent_hash: &H256) -> Arc<Vec<H256>> {
        let mut last_hashes = vec![];
        last_hashes.resize(256, H256::default());
        last_hashes[0] = parent_hash.clone();
        for i in 0..255 {
            match chain.block_details(&last_hashes[i]) {
                Some(details) => {
                    last_hashes[i + 1] = details.parent.clone();
                }
                None => break,
            }
        }

        Arc::new(last_hashes)
    }

    pub fn get_account_storage(
        &self,
        ctx: Arc<IoContext>,
        address: Address,
        key: H256,
    ) -> Fallible<H256> {
        Ok(self.get_state(ctx)?.storage_at(&address, &key)?)
    }

    pub fn get_account_nonce(&self, ctx: Arc<IoContext>, address: &Address) -> Fallible<U256> {
        Ok(self.get_state(ctx)?.nonce(&address)?)
    }

    pub fn get_account_balance(&self, ctx: Arc<IoContext>, address: &Address) -> Fallible<U256> {
        Ok(self.get_state(ctx)?.balance(&address)?)
    }

    pub fn get_account_code(
        &self,
        ctx: Arc<IoContext>,
        address: &Address,
    ) -> Fallible<Option<Vec<u8>>> {
        // convert from Option<Arc<Vec<u8>>> to Option<Vec<u8>>
        Ok(self.get_state(ctx)?.code(&address)?.map(|c| (&*c).clone()))
    }

    pub fn get_storage_expiry(&self, ctx: Arc<IoContext>, address: &Address) -> Fallible<u64> {
        Ok(self.get_state(ctx)?.storage_expiry(&address)?)
    }

    fn block_number_ref_chain(&self, chain: &BlockChain, id: &BlockId) -> Option<BlockNumber> {
        match *id {
            BlockId::Number(number) => Some(number),
            BlockId::Hash(ref hash) => chain.block_number(hash),
            BlockId::Earliest => Some(0),
            BlockId::Latest => Some(chain.best_block_number()),
        }
    }

    pub fn get_logs(&self, filter: &Filter) -> Fallible<Vec<Log>> {
        let inner = self.inner.lock().unwrap();
        let chain = inner.chain.as_ref().ok_or(CacheError::NotInitialized)?;

        let filter = EthcoreFilter {
            from_block: util::to_block_id(filter.from_block.clone()),
            to_block: util::to_block_id(filter.to_block.clone()),
            address: match filter.address.clone() {
                Some(address) => Some(address.into_iter().map(Into::into).collect()),
                None => None,
            },
            topics: filter.topics.clone().into_iter().map(Into::into).collect(),
            limit: filter.limit.map(Into::into),
        };

        // if either the from or to block is invalid, return an empty Vec
        let from = match self.block_number_ref_chain(&chain, &filter.from_block) {
            Some(n) => n,
            None => return Ok(vec![]),
        };
        let to = match self.block_number_ref_chain(&chain, &filter.to_block) {
            Some(n) => n,
            None => return Ok(vec![]),
        };

        let blocks = filter
            .bloom_possibilities()
            .iter()
            .map(|bloom| chain.blocks_with_bloom(bloom, from, to))
            .flat_map(|m| m)
            // remove duplicate elements
            .collect::<HashSet<u64>>()
            .into_iter()
            .filter_map(|n| chain.block_hash(n))
            .collect::<Vec<H256>>();

        Ok(chain
            .logs(blocks, |entry| filter.matches(entry), filter.limit)
            .into_iter()
            .map(util::lle_to_log)
            .collect())
    }

    pub fn add_block(&self, block: LockedBlock) -> Fallible<()> {
        let inner = self.inner.lock().unwrap();
        let chain = inner.chain.as_ref().ok_or(CacheError::NotInitialized)?;

        debug!(self.logger, "Finalizing block");

        let block = block.seal(&*genesis::SPEC.engine, Vec::new())?;

        // Queue the db operations necessary to insert this block.
        let mut db_tx = kvdb::DBTransaction::default();
        chain.insert_block(
            &mut db_tx,
            &block.rlp_bytes(),
            block.receipts().to_owned(),
            ExtrasInsert {
                fork_choice: ForkChoice::New,
                is_finalized: true,
                metadata: None,
            },
        );

        // Commit the insert to the in-memory blockchain cache.
        chain.commit();
        // Write hash db updates.
        self.hash_db.commit();
        // Write blockchain updates.
        self.state_db
            .write(db_tx)
            .expect("write blockchain updates");

        debug!(self.logger, "Block finalized");

        Ok(())
    }

    pub fn get_transaction(&self, hash: &H256) -> Fallible<Option<Transaction>> {
        let inner = self.inner.lock().unwrap();
        let chain = inner.chain.as_ref().ok_or(CacheError::NotInitialized)?;

        let addr = match chain.transaction_address(hash) {
            Some(addr) => addr,
            None => return Ok(None),
        };
        let mut tx = match chain.transaction(&addr) {
            Some(tx) => tx,
            None => return Ok(None),
        };
        let signature = tx.signature();

        Ok(Some(Transaction {
            hash: tx.hash(),
            nonce: tx.nonce,
            block_hash: Some(tx.block_hash),
            block_number: Some(U256::from(tx.block_number)),
            index: Some(tx.transaction_index.into()),
            from: tx.sender(),
            to: match tx.action {
                Action::Create => None,
                Action::Call(address) => Some(address),
            },
            value: tx.value,
            gas_price: tx.gas_price,
            gas: tx.gas,
            input: tx.data.clone(),
            creates: match tx.action {
                Action::Create => Some(util::get_contract_address(&tx.sender(), &tx)),
                Action::Call(_) => None,
            },
            raw: rlp::encode(&tx.signed).into_vec(),
            // TODO: recover pubkey
            public_key: None,
            chain_id: tx.chain_id().into(),
            standard_v: tx.standard_v().into(),
            v: tx.original_v().into(),
            r: signature.r().into(),
            s: signature.s().into(),
        }))
    }

    pub fn get_receipt(&self, hash: &H256) -> Fallible<Option<Receipt>> {
        let inner = self.inner.lock().unwrap();
        let chain = inner.chain.as_ref().ok_or(CacheError::NotInitialized)?;

        let addr = match chain.transaction_address(hash) {
            Some(addr) => addr,
            None => return Ok(None),
        };
        let mut tx = match chain.transaction(&addr) {
            Some(tx) => tx,
            None => return Ok(None),
        };
        let receipt = match chain.transaction_receipt(&addr) {
            Some(receipt) => receipt,
            None => return Ok(None),
        };

        Ok(Some(Receipt {
            hash: Some(tx.hash()),
            index: Some(U256::from(addr.index)),
            block_hash: Some(tx.block_hash),
            block_number: Some(U256::from(tx.block_number)),
            cumulative_gas_used: receipt.gas_used, // TODO: get from block header
            gas_used: Some(receipt.gas_used),
            contract_address: match tx.action {
                Action::Create => Some(util::get_contract_address(&tx.sender(), &tx)),
                Action::Call(_) => None,
            },
            logs: receipt.logs.into_iter().map(util::le_to_log).collect(),
            logs_bloom: receipt.log_bloom,
            state_root: match receipt.outcome {
                TransactionOutcome::StateRoot(hash) => Some(hash),
                _ => None,
            },
            status_code: match receipt.outcome {
                TransactionOutcome::StatusCode(code) => Some(code.into()),
                _ => None,
            },
        }))
    }

    pub fn block_hash(&self, number: BlockNumber) -> Fallible<Option<H256>> {
        let inner = self.inner.lock().unwrap();
        let chain = inner.chain.as_ref().ok_or(CacheError::NotInitialized)?;

        Ok(chain.block_hash(number))
    }

    pub fn block_by_number(&self, number: BlockNumber) -> Fallible<Option<Block>> {
        let inner = self.inner.lock().unwrap();
        let chain = inner.chain.as_ref().ok_or(CacheError::NotInitialized)?;

        Ok(chain.block_hash(number).and_then(|hash| chain.block(&hash)))
    }

    pub fn block_by_hash(&self, hash: H256) -> Fallible<Option<Block>> {
        let inner = self.inner.lock().unwrap();
        let chain = inner.chain.as_ref().ok_or(CacheError::NotInitialized)?;

        Ok(chain.block(&hash))
    }

    pub fn get_latest_block_number(&self) -> Fallible<BlockNumber> {
        let inner = self.inner.lock().unwrap();
        let chain = inner.chain.as_ref().ok_or(CacheError::NotInitialized)?;

        Ok(chain.best_block_number())
    }

    pub fn best_block_header(&self) -> Fallible<Header> {
        let inner = self.inner.lock().unwrap();
        let chain = inner.chain.as_ref().ok_or(CacheError::NotInitialized)?;

        Ok(chain.best_block_header())
    }
}

/// CAS implementation which uses the thread-local CAS provided by
/// the `StorageContext`.
struct ThreadLocalCAS;

impl CAS for ThreadLocalCAS {
    fn get(&self, key: Hash) -> Fallible<Vec<u8>> {
        StorageContext::with_current(|cas, _mkvs| cas.get(key))
    }

    fn insert(&self, value: Vec<u8>, expiry: u64) -> Fallible<Hash> {
        StorageContext::with_current(|cas, _mkvs| cas.insert(value, expiry))
    }
}

/// MKVS implementation which uses the thread-local MKVS provided by
/// the `StorageContext`.
struct ThreadLocalMKVS;

impl MKVS for ThreadLocalMKVS {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        StorageContext::with_current(|_cas, mkvs| mkvs.get(key))
    }

    fn insert(&mut self, key: &[u8], value: &[u8]) -> Option<Vec<u8>> {
        StorageContext::with_current(|_cas, mkvs| mkvs.insert(key, value))
    }

    fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        StorageContext::with_current(|_cas, mkvs| mkvs.remove(key))
    }

    fn commit(&mut self) -> Fallible<Hash> {
        StorageContext::with_current(|_cas, mkvs| mkvs.commit())
    }

    fn rollback(&mut self) {
        StorageContext::with_current(|_cas, mkvs| mkvs.rollback())
    }

    fn set_encryption_key(&mut self, key: Option<&[u8]>) {
        StorageContext::with_current(|_cas, mkvs| mkvs.set_encryption_key(key))
    }
}
