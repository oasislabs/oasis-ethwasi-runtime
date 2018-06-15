use ethereum_types::{H256, U256};
use evm_api::Block;
use sha3::{Digest, Keccak256};
use state::StateDb;

pub struct BlockHashes {
  tx_hash: H256,
  state_root: H256,
}

// "mine" a block containing 0 or 1 transactions
// returns block number and hash
pub fn mine_block(transaction_hash: Option<H256>, state_root: H256) -> (U256, H256) {
  // get the next block number
  let block_number = incr_block_number();

  // create a block
  let transaction_hash = transaction_hash.unwrap_or(H256::zero());

  // set parent hash
  let parent_hash = if block_number > U256::zero() {
    get_block(block_number - U256::one()).unwrap().hash
  } else {
    // genesis block
    H256::zero()
  };

  // compute a unique block hash
  // WARNING: the value is deterministic and guessable!
  let block_hash = H256::from(
    Keccak256::digest_str(&format!(
      "{:x} {:x} {:x}",
      block_number, transaction_hash, parent_hash
    )).as_slice(),
  );

  let block = Block {
    number: block_number,
    parent_hash: parent_hash,
    hash: block_hash,
    state_root: state_root,
    transaction_hash: transaction_hash,
    transaction: None,
  };

  // store the block
  StateDb::new().blocks.insert(&block_number, &block);;

  (block_number, block_hash)
}

pub fn get_block(number: U256) -> Option<Block> {
  let state = StateDb::new();
  state.blocks.get(&number)
}

pub fn get_latest_block_number() -> U256 {
  StateDb::new()
    .latest_block_number
    .get()
    .unwrap_or(U256::zero())
}

pub fn get_latest_block() -> Option<Block> {
  get_block(get_latest_block_number())
}

/// Increments the block number and returns the new block number.
fn incr_block_number() -> U256 {
  let state = StateDb::new();

  let next = if state.latest_block_number.is_present() {
    state.latest_block_number.get().unwrap() + U256::one()
  } else {
    // genesis block
    U256::zero()
  };

  //  store new value
  state.latest_block_number.insert(&next);
  next
}
