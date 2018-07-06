use std::{cmp,
          collections::BTreeMap,
          io::{Cursor, Read},
          mem,
          sync::Arc};

use ekiden_core::error::Result;
use ekiden_trusted::db::{database_schema, Database, DatabaseHandle};
use ethcore::{self,
              block::{Block, Drain, IsBlock, LockedBlock, OpenBlock, SealedBlock},
              blockchain::{BlockChain, BlockProvider, ExtrasInsert},
              engines::{ForkChoice, InstantSeal},
              executed::Executed,
              header::Header,
              journaldb::overlaydb::OverlayDB,
              kvdb::{self, KeyValueDB},
              machine::EthereumMachine,
              rlp::{decode, Decodable},
              spec::{CommonParams, Spec},
              state::backend::Basic as BasicBackend,
              transaction::{Action, SignedTransaction},
              types::{receipt::{Receipt, TransactionOutcome},
                      BlockNumber}};
use ethereum_types::{Address, H256, U256};
use evm_api::{AccountState, TransactionRecord};

use super::{evm::get_contract_address, util::to_hex};

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
    info!("GOT BACKEND");
    println!("{:?}", CHAIN.best_block_header());
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
    println!("{:?}", parent);
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

// returns a hex-encoded string directly from storage to avoid unnecessary conversions
pub fn get_code_string(address: &Address) -> Result<String> {
    Ok(get_code_string_from_state(&get_state()?, address)?)
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
        if head.number() >= start {
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
    db.commit_to_batch(&mut db_tx); // add any pending state updates to the db transaction
    StateDb::instance().write(db_tx); // persist the changes to the backing db

    Ok(())
}

pub fn get_transaction_record(hash: &H256) -> Option<TransactionRecord> {
    CHAIN.transaction_address(hash).map(|addr| {
        let mut tx = CHAIN.transaction(&addr).unwrap();
        let receipt = CHAIN.transaction_receipt(&addr).unwrap();
        TransactionRecord {
            hash: tx.hash(),
            nonce: tx.nonce,
            block_hash: tx.block_hash,
            block_number: U256::from(tx.block_number),
            index: addr.index,
            is_create: tx.action == Action::Create,
            from: tx.sender(),
            to: match tx.action {
                Action::Create => None,
                Action::Call(address) => Some(address),
            },
            contract_address: match tx.action {
                Action::Create => Some(get_contract_address(&tx)),
                Action::Call(_) => None,
            },
            input: to_hex(&tx.data),
            value: tx.value,
            gas_price: tx.gas_price,
            gas_provided: tx.gas,
            gas_used: receipt.gas_used,
            cumulative_gas_used: receipt.gas_used, // TODO: get from block header
            exited_ok: match receipt.outcome {
                TransactionOutcome::StatusCode(code) => code == 1,
                _ => false,
            },
            logs: receipt.logs,
        }
    })
}

pub fn get_block_hash(number: BlockNumber) -> Option<H256> {
    CHAIN.block_hash(number)
}

pub fn block_by_number(number: BlockNumber) -> Option<Block> {
    CHAIN
        .block_hash(number)
        .and_then(|hash| CHAIN.block(&hash))
        .map(|enc| enc.decode().unwrap())
}

pub fn block_by_hash(hash: H256) -> Option<Block> {
    CHAIN.block(&hash).map(|encoded| encoded.decode().unwrap())
}

pub fn get_latest_block() -> Option<Block> {
    block_by_number(get_latest_block_number())
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
