use std::{collections::HashSet, io::Cursor, mem, sync::Arc};

use ekiden_core::error::Result;
use ekiden_trusted::db::{Database, DatabaseHandle};
use ethcore::{self,
              block::{Drain, IsBlock, LockedBlock, OpenBlock},
              blockchain::{BlockChain, BlockProvider, ExtrasInsert},
              encoded::Block,
              engines::ForkChoice,
              filter::Filter as EthcoreFilter,
              journaldb::overlaydb::OverlayDB,
              kvdb::{self, KeyValueDB},
              spec::Spec,
              state::backend::Basic as BasicBackend,
              transaction::Action,
              types::{ids::BlockId,
                      log_entry::{LocalizedLogEntry, LogEntry},
                      receipt::TransactionOutcome,
                      BlockNumber}};
use ethereum_api::{AccountState, BlockId as EkidenBlockId, Filter, Log, Receipt, Transaction};
use ethereum_types::{Address, H256, U256};
use hex;

use super::evm::get_contract_address;

lazy_static! {
  static ref SPEC: Spec = {
    #[cfg(not(feature = "benchmark"))]
    let spec_json = include_str!("../resources/genesis/genesis.json");
    #[cfg(feature = "benchmark")]
    let spec_json = include_str!("../resources/genesis/genesis_benchmarking.json");
    Spec::load(Cursor::new(spec_json)).unwrap()
  };
  static ref CHAIN: BlockChain = {
    let mut db = SPEC.ensure_db_good(get_backend(), &Default::default() /* factories */).unwrap();
    db.0.commit().unwrap();

    BlockChain::new(
      Default::default() /* config */,
      &*SPEC.genesis_block(),
      Arc::new(StateDb::instance())
    )
  };
}

pub struct StateDb {}

type Backend = BasicBackend<OverlayDB>;
type State = ethcore::state::State<Backend>;

pub(crate) fn get_backend() -> Backend {
    BasicBackend(OverlayDB::new(
        Arc::new(StateDb::instance()),
        None, /* col */
    ))
}

pub(crate) fn get_state() -> Result<State> {
    let backend = get_backend();
    let root = CHAIN.best_block_header().state_root().clone();
    Ok(ethcore::state::State::from_existing(
        backend,
        root,
        U256::zero(),       /* account_start_nonce */
        Default::default(), /* factories */
    )?)
}

pub(crate) fn new_block() -> Result<OpenBlock<'static>> {
    let parent = CHAIN.best_block_header();
    Ok(OpenBlock::new(
        &*SPEC.engine,
        Default::default(),                                     /* factories */
        false,                                                  /* tracing */
        get_backend(),                                          /* state_db */
        &parent,                                                /* parent */
        Arc::new(block_hashes_since(BlockOffset::Offset(256))), /* last hashes */
        Address::default(),                                     /* author */
        (U256::one(), U256::max_value()),                       /* gas_range_target */
        vec![],                                                 /* extra data */
        true,                                                   /* is epoch_begin */
        &mut Vec::new().into_iter(),                            /* ancestry */
    )?)
}

pub fn with_state<R, F: FnOnce(&mut State) -> Result<R>>(cb: F) -> Result<(R, H256)> {
    let mut state = get_state()?;

    let ret = cb(&mut state)?;

    state.commit()?;
    let (state_root, mut db) = state.drop();
    db.0.commit()?;

    Ok((ret, state_root))
}

impl StateDb {
    fn new() -> Self {
        Self {}
    }

    pub fn instance() -> Self {
        Self::new()
    }
}

fn to_hex<T: AsRef<Vec<u8>>>(bytes: T) -> String {
    hex::encode(bytes.as_ref())
}

pub fn get_account_state(address: &Address) -> Result<Option<AccountState>> {
    let state = get_state()?;
    if !state.exists_and_not_null(address)? {
        return Ok(None);
    }
    Ok(Some(AccountState {
        address: address.clone(),
        nonce: state.nonce(address)?,
        balance: state.balance(address)?,
        code: get_code_string_from_state(&state, address)?,
    }))
}

fn get_code_string_from_state(state: &State, address: &Address) -> Result<String> {
    Ok(state.code(address)?.map(to_hex).unwrap_or(String::new()))
}

pub fn get_account_storage(address: Address, key: H256) -> Result<H256> {
    Ok(get_state()?.storage_at(&address, &key)?)
}

pub fn get_account_nonce(address: &Address) -> Result<U256> {
    Ok(get_state()?.nonce(&address)?)
}

pub fn get_account_balance(address: &Address) -> Result<U256> {
    Ok(get_state()?.balance(&address)?)
}

pub fn get_account_code(address: &Address) -> Result<Option<Vec<u8>>> {
    // convert from Option<Arc<Vec<u8>>> to Option<Vec<u8>>
    Ok(get_state()?.code(&address)?.map(|c| (&*c).clone()))
}

fn block_number_ref(id: &BlockId) -> Option<BlockNumber> {
    match *id {
        BlockId::Number(number) => Some(number),
        BlockId::Hash(ref hash) => CHAIN.block_number(hash),
        BlockId::Earliest => Some(0),
        BlockId::Latest => Some(CHAIN.best_block_number()),
    }
}

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

fn to_block_id(id: EkidenBlockId) -> BlockId {
    match id {
        EkidenBlockId::Number(number) => BlockId::Number(number.into()),
        EkidenBlockId::Hash(hash) => BlockId::Hash(hash),
        EkidenBlockId::Earliest => BlockId::Earliest,
        EkidenBlockId::Latest => BlockId::Latest,
    }
}

pub fn get_logs(filter: &Filter) -> Vec<Log> {
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

    let from = block_number_ref(&filter.from_block).unwrap();
    let to = block_number_ref(&filter.to_block).unwrap();

    let blocks = filter.bloom_possibilities().iter()
        .map(|bloom| {
            CHAIN.blocks_with_bloom(bloom, from, to)
        })
    .flat_map(|m| m)
        // remove duplicate elements
        .collect::<HashSet<u64>>()
        .into_iter()
        .filter_map(|n| CHAIN.block_hash(n))
        .collect::<Vec<H256>>();

    CHAIN
        .logs(blocks, |entry| filter.matches(entry), filter.limit)
        .into_iter()
        .map(lle_to_log)
        .collect()
}

pub enum BlockOffset {
    Offset(u64),
    Absolute(u64),
}

pub fn block_hashes_since(start: BlockOffset) -> Vec<H256> {
    let mut head = CHAIN.best_block_header();

    let start = match start {
        BlockOffset::Offset(offset) => if head.number() < offset {
            0
        } else {
            head.number() - offset
        },
        BlockOffset::Absolute(num) => if num <= head.number() {
            num
        } else {
            return Vec::new();
        },
    };
    let mut hashes = Vec::with_capacity((head.number() - start + 1) as usize);

    loop {
        hashes.push(head.hash());
        if head.number() <= start {
            break;
        }
        head = CHAIN
            .block_header_data(head.parent_hash())
            .map(|enc| enc.decode().unwrap())
            .expect("Parent block should exist?");
    }

    hashes
}

pub fn add_block(block: LockedBlock) -> Result<()> {
    let block = block.seal(&*SPEC.engine, Vec::new())?;

    let mut db_tx = kvdb::DBTransaction::default();

    // queue the db ops necessary to insert this block
    CHAIN.insert_block(
        &mut db_tx,
        &block.rlp_bytes(),
        block.receipts().to_owned(),
        ExtrasInsert {
            fork_choice: ForkChoice::New,
            is_finalized: true,
            metadata: None,
        },
    );

    CHAIN.commit(); // commit the insert to the in-memory BlockChain repr
    let mut db = block.drain().0;
    db.commit_to_batch(&mut db_tx)
        .expect("could not commit state updates"); // add any pending state updates to the db transaction
    StateDb::instance()
        .write(db_tx)
        .expect("could not persist state updates"); // persist the changes to the backing db

    Ok(())
}

pub fn get_transaction(hash: &H256) -> Option<Transaction> {
    CHAIN.transaction_address(hash).map(|addr| {
        let mut tx = CHAIN.transaction(&addr).unwrap();
        let signature = tx.signature();
        Transaction {
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
                Action::Create => Some(get_contract_address(&tx)),
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
        }
    })
}

pub fn get_receipt(hash: &H256) -> Option<Receipt> {
    CHAIN.transaction_address(hash).map(|addr| {
        let tx = CHAIN.transaction(&addr).unwrap();
        let receipt = CHAIN.transaction_receipt(&addr).unwrap();
        Receipt {
            hash: Some(tx.hash()),
            index: Some(U256::from(addr.index)),
            block_hash: Some(tx.block_hash),
            block_number: Some(U256::from(tx.block_number)),
            cumulative_gas_used: receipt.gas_used, // TODO: get from block header
            gas_used: Some(receipt.gas_used),
            contract_address: match tx.action {
                Action::Create => Some(get_contract_address(&tx)),
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
        }
    })
}

pub fn block_hash(number: BlockNumber) -> Option<H256> {
    CHAIN.block_hash(number)
}

pub fn block_by_number(number: BlockNumber) -> Option<Block> {
    CHAIN.block_hash(number).and_then(|hash| CHAIN.block(&hash))
}

pub fn block_by_hash(hash: H256) -> Option<Block> {
    CHAIN.block(&hash)
}

pub fn get_latest_block_number() -> BlockNumber {
    CHAIN.best_block_number()
}

pub fn to_bytes(num: u32) -> [u8; mem::size_of::<u32>()] {
    unsafe { mem::transmute(num) }
}

// parity expects the database to namespace keys by column
// the ekiden db doesn't [yet?] have this feature, so we emulate by
// prepending the column id to the actual key
fn get_key(col: Option<u32>, key: &[u8]) -> Vec<u8> {
    let col_bytes = col.map(|id| to_bytes(id.to_le())).unwrap_or([0, 0, 0, 0]);
    col_bytes
        .into_iter()
        .chain(key.into_iter())
        .map(|v| v.to_owned())
        .collect()
}

impl kvdb::KeyValueDB for StateDb {
    fn get(&self, col: Option<u32>, key: &[u8]) -> kvdb::Result<Option<kvdb::DBValue>> {
        Ok(DatabaseHandle::instance()
            .get(&get_key(col, key))
            .map(kvdb::DBValue::from_vec))
    }

    fn get_by_prefix(&self, _col: Option<u32>, _prefix: &[u8]) -> Option<Box<[u8]>> {
        unimplemented!();
    }

    fn write_buffered(&self, transaction: kvdb::DBTransaction) {
        transaction.ops.iter().for_each(|op| match op {
            &kvdb::DBOp::Insert {
                ref key,
                ref value,
                col,
            } => {
                DatabaseHandle::instance().insert(&get_key(col, key), value.to_vec().as_slice());
            }
            &kvdb::DBOp::Delete { ref key, col } => {
                DatabaseHandle::instance().remove(&get_key(col, key));
            }
        });
    }

    fn flush(&self) -> kvdb::Result<()> {
        Ok(())
    }

    fn iter<'a>(&'a self, _col: Option<u32>) -> Box<Iterator<Item = (Box<[u8]>, Box<[u8]>)> + 'a> {
        unimplemented!();
    }

    fn iter_from_prefix<'a>(
        &'a self,
        _col: Option<u32>,
        _prefix: &'a [u8],
    ) -> Box<Iterator<Item = (Box<[u8]>, Box<[u8]>)> + 'a> {
        unimplemented!();
    }

    fn restore(&self, _new_db: &str) -> kvdb::Result<()> {
        unimplemented!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazy_static;

    #[test]
    fn test_create_chain() {
        lazy_static::initialize(&CHAIN);
    }
}
