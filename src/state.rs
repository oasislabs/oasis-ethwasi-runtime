use std::{collections::HashSet,
          sync::{Arc, Mutex}};

use super::evm::{get_contract_address, GAS_LIMIT, SPEC};
use ekiden_core::{self, error::Result};
use ekiden_storage_base::StorageBackend;
use ekiden_trusted::db::{Database, DatabaseHandle};
use ethcore::{self,
              block::{IsBlock, LockedBlock, OpenBlock},
              blockchain::{BlockChain, BlockProvider, ExtrasInsert},
              encoded::Block,
              engines::ForkChoice,
              filter::Filter as EthcoreFilter,
              header::Header,
              kvdb::{self, KeyValueDB},
              state::backend::Wrapped as WrappedBackend,
              transaction::Action,
              types::{ids::BlockId,
                      log_entry::{LocalizedLogEntry, LogEntry},
                      receipt::TransactionOutcome,
                      BlockNumber}};
use ethereum_api::{BlockId as EkidenBlockId, Filter, Log, Receipt, Transaction};
use ethereum_types::{Address, H256, U256};
use runtime_ethereum_common::{confidential::ConfidentialCtx, get_factories, Backend,
                              BlockchainStateDb, State, StorageHashDB};

lazy_static! {
    static ref GLOBAL_CACHE: Mutex<Option<Cache>> = Mutex::new(None);
}

/// Cache is the in-memory blockchain cache backed by the database.
pub struct Cache {
    /// Blockchain state database instance.
    blockchain_db: Arc<BlockchainStateDb<DatabaseHandle>>,
    /// Ethereum state backend.
    state_backend: Backend,
    /// Ethereum state database.
    state_db: StorageHashDB<DatabaseHandle>,
    /// Actual blockchain cache.
    chain: BlockChain,
}

impl Cache {
    /// Create a new in-memory cache for the given state root.
    pub fn new(storage: Arc<StorageBackend>, db: DatabaseHandle) -> Self {
        let blockchain_db = Arc::new(BlockchainStateDb::new(db));
        let state_db = StorageHashDB::new(storage, blockchain_db.clone());
        // Initialize Ethereum state with the genesis block in case there is none.
        let state_backend =
            SPEC.ensure_db_good(WrappedBackend(Box::new(state_db.clone())), &get_factories())
                .expect("state to be initialized");
        state_db.commit();

        Self {
            blockchain_db: blockchain_db.clone(),
            state_backend,
            state_db,
            chain: Self::new_chain(blockchain_db),
        }
    }

    /// Fetches a global `Cache` instance for the given state root.
    ///
    /// In case the current global instance is not valid for the given state root,
    /// it will be replaced.
    pub fn from_global(storage: Arc<StorageBackend>, db: DatabaseHandle) -> Cache {
        let mut maybe_cache = GLOBAL_CACHE.lock().unwrap();
        match maybe_cache.take() {
            Some(cache) => {
                if cache.blockchain_db.get_root_hash() != db.get_root_hash() {
                    // Root hash differs, re-create the cache from scratch.
                    Cache::new(storage, db)
                } else {
                    cache
                }
            }
            None => {
                // No cache is available, create one.
                Cache::new(storage, db)
            }
        }
    }

    /// Commit changes to global `Cache` instance.
    pub fn commit_global(self) -> ekiden_core::bytes::H256 {
        let mut maybe_cache = GLOBAL_CACHE.lock().unwrap();
        if maybe_cache.is_some() {
            panic!("Multiple concurrent cache commits");
        }

        // Commit any pending state updates.
        self.state_db.commit();
        // Commit any blockchain state updates.
        let root_hash = self.blockchain_db
            .commit()
            .expect("commit blockchain state");

        *maybe_cache = Some(self);

        root_hash
    }

    fn new_chain(blockchain_db: Arc<BlockchainStateDb<DatabaseHandle>>) -> BlockChain {
        BlockChain::new(
            Default::default(), /* config */
            &*SPEC.genesis_block(),
            blockchain_db,
        )
    }

    pub fn get_state(&self, ctx: ConfidentialCtx) -> Result<State> {
        let root = self.chain.best_block_header().state_root().clone();
        Ok(ethcore::state::State::from_existing(
            self.state_backend.clone(),
            root,
            U256::zero(), /* account_start_nonce */
            get_factories(),
            Some(Box::new(ctx)),
        )?)
    }

    pub(crate) fn new_block(&self) -> Result<OpenBlock<'static>> {
        let parent = self.chain.best_block_header();
        Ok(OpenBlock::new(
            &*SPEC.engine,
            get_factories(),
            cfg!(debug_assertions),           /* tracing */
            self.state_backend.clone(),       /* state_db */
            &parent,                          /* parent */
            self.last_hashes(&parent.hash()), /* last hashes */
            Address::default(),               /* author */
            *GAS_LIMIT,                       /* block gas limit */
            vec![],                           /* extra data */
            true,                             /* is epoch_begin */
            &mut Vec::new().into_iter(),      /* ancestry */
            Some(Box::new(ConfidentialCtx::new())),
        )?)
    }

    pub fn get_account_storage(&self, address: Address, key: H256) -> Result<H256> {
        Ok(self.get_state(ConfidentialCtx::new())?.storage_at(&address, &key)?)
    }

    pub fn get_account_nonce(&self, address: &Address) -> Result<U256> {
        Ok(self.get_state(ConfidentialCtx::new())?.nonce(&address)?)
    }

    pub fn get_account_balance(&self, address: &Address) -> Result<U256> {
        Ok(self.get_state(ConfidentialCtx::new())?.balance(&address)?)
    }

    pub fn get_account_code(&self, address: &Address) -> Result<Option<Vec<u8>>> {
        // convert from Option<Arc<Vec<u8>>> to Option<Vec<u8>>
        Ok(self.get_state(ConfidentialCtx::new())?.code(&address)?.map(|c| (&*c).clone()))
    }

    fn block_number_ref(&self, id: &BlockId) -> Option<BlockNumber> {
        match *id {
            BlockId::Number(number) => Some(number),
            BlockId::Hash(ref hash) => self.chain.block_number(hash),
            BlockId::Earliest => Some(0),
            BlockId::Latest => Some(self.chain.best_block_number()),
        }
    }

    pub fn get_logs(&self, filter: &Filter) -> Vec<Log> {
        let filter = EthcoreFilter {
            from_block: to_block_id(filter.from_block.clone()),
            to_block: to_block_id(filter.to_block.clone()),
            address: match filter.address.clone() {
                Some(address) => Some(address.into_iter().map(Into::into).collect()),
                None => None,
            },
            topics: filter.topics.clone().into_iter().map(Into::into).collect(),
            limit: filter.limit.map(Into::into),
        };

        // if either the from or to block is invalid, return an empty Vec
        let from = match self.block_number_ref(&filter.from_block) {
            Some(n) => n,
            None => return vec![],
        };
        let to = match self.block_number_ref(&filter.to_block) {
            Some(n) => n,
            None => return vec![],
        };

        let blocks = filter.bloom_possibilities().iter()
            .map(|bloom| {
                self.chain.blocks_with_bloom(bloom, from, to)
            })
        .flat_map(|m| m)
            // remove duplicate elements
            .collect::<HashSet<u64>>()
            .into_iter()
            .filter_map(|n| self.chain.block_hash(n))
            .collect::<Vec<H256>>();

        self.chain
            .logs(blocks, |entry| filter.matches(entry), filter.limit)
            .into_iter()
            .map(lle_to_log)
            .collect()
    }

    pub fn last_hashes(&self, parent_hash: &H256) -> Arc<Vec<H256>> {
        let mut last_hashes = vec![];
        last_hashes.resize(256, H256::default());
        last_hashes[0] = parent_hash.clone();
        for i in 0..255 {
            match self.chain.block_details(&last_hashes[i]) {
                Some(details) => {
                    last_hashes[i + 1] = details.parent.clone();
                }
                None => break,
            }
        }
        Arc::new(last_hashes)
    }

    pub fn add_block(&mut self, block: LockedBlock) -> Result<()> {
        let block = block.seal(&*SPEC.engine, Vec::new())?;

        // Queue the db operations necessary to insert this block.
        let mut db_tx = kvdb::DBTransaction::default();
        self.chain.insert_block(
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
        self.chain.commit();
        // Write blockchain updates.
        self.blockchain_db
            .write(db_tx)
            .expect("write blockchain updates");

        Ok(())
    }

    pub fn get_transaction(&self, hash: &H256) -> Option<Transaction> {
        let addr = self.chain.transaction_address(hash)?;
        let mut tx = self.chain.transaction(&addr)?;
        let signature = tx.signature();
        Some(Transaction {
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
                Action::Create => Some(get_contract_address(&tx.sender(), &tx)),
                Action::Call(_) => None,
            },
            raw: ::rlp::encode(&tx.signed).into_vec(),
            // TODO: recover pubkey
            public_key: None,
            chain_id: tx.chain_id().into(),
            standard_v: tx.standard_v().into(),
            v: tx.original_v().into(),
            r: signature.r().into(),
            s: signature.s().into(),
        })
    }

    pub fn get_receipt(&self, hash: &H256) -> Option<Receipt> {
        let addr = self.chain.transaction_address(hash)?;
        let mut tx = self.chain.transaction(&addr)?;
        let receipt = self.chain.transaction_receipt(&addr)?;
        Some(Receipt {
            hash: Some(tx.hash()),
            index: Some(U256::from(addr.index)),
            block_hash: Some(tx.block_hash),
            block_number: Some(U256::from(tx.block_number)),
            cumulative_gas_used: receipt.gas_used, // TODO: get from block header
            gas_used: Some(receipt.gas_used),
            contract_address: match tx.action {
                Action::Create => Some(get_contract_address(&tx.sender(), &tx)),
                Action::Call(_) => None,
            },
            logs: receipt.logs.into_iter().map(le_to_log).collect(),
            logs_bloom: receipt.log_bloom,
            state_root: match receipt.outcome {
                TransactionOutcome::StateRoot(hash) => Some(hash),
                _ => None,
            },
            status_code: match receipt.outcome {
                TransactionOutcome::StatusCode(code) => Some(code.into()),
                _ => None,
            },
        })
    }

    pub fn block_hash(&self, number: BlockNumber) -> Option<H256> {
        self.chain.block_hash(number)
    }

    pub fn block_by_number(&self, number: BlockNumber) -> Option<Block> {
        self.chain
            .block_hash(number)
            .and_then(|hash| self.chain.block(&hash))
    }

    pub fn block_by_hash(&self, hash: H256) -> Option<Block> {
        self.chain.block(&hash)
    }

    pub fn get_latest_block_number(&self) -> BlockNumber {
        self.chain.best_block_number()
    }

    pub fn best_block_header(&self) -> Header {
        self.chain.best_block_header()
    }
}

// TODO: Move to util module.
fn lle_to_log(lle: LocalizedLogEntry) -> Log {
    Log {
        address: lle.entry.address,
        topics: lle.entry.topics.into_iter().map(Into::into).collect(),
        data: lle.entry.data.into(),
        block_hash: Some(lle.block_hash),
        block_number: Some(lle.block_number.into()),
        transaction_hash: Some(lle.transaction_hash),
        transaction_index: Some(lle.transaction_index.into()),
        log_index: Some(lle.log_index.into()),
        transaction_log_index: Some(lle.transaction_log_index.into()),
    }
}

// TODO: Move to util module.
fn le_to_log(le: LogEntry) -> Log {
    Log {
        address: le.address,
        topics: le.topics.into_iter().map(Into::into).collect(),
        data: le.data.into(),
        block_hash: None,
        block_number: None,
        transaction_hash: None,
        transaction_index: None,
        log_index: None,
        transaction_log_index: None,
    }
}

// TODO: Move to util module.
fn to_block_id(id: EkidenBlockId) -> BlockId {
    match id {
        EkidenBlockId::Number(number) => BlockId::Number(number.into()),
        EkidenBlockId::Hash(hash) => BlockId::Hash(hash),
        EkidenBlockId::Earliest => BlockId::Earliest,
        EkidenBlockId::Latest => BlockId::Latest,
    }
}

#[cfg(test)]
mod tests {
    extern crate ekiden_storage_dummy;

    use self::ekiden_storage_dummy::DummyStorageBackend;
    use lazy_static;

    use super::*;

    #[test]
    fn test_create_chain() {
        let storage = Arc::new(DummyStorageBackend::new());

        Cache::new(storage.clone(), DatabaseHandle::new(storage));
    }
}
