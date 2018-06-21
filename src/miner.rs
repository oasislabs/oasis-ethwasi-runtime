use ethereum_types::{H256, U256};
use evm_api::Block;
use sha3::{Digest, Keccak256};

use state::{block_by_number, get_latest_block_number, set_block};

/// "mine" a block containing 0 or 1 transactions.
/// Returns block number and hash.
pub fn mine_block(transaction_hash: Option<H256>, state_root: H256) -> (U256, H256) {
  // get the next block number
  let block_number = if transaction_hash.is_some() {
    get_latest_block_number() + U256::one()
  } else {
    U256::zero()
  };

  // create a block
  let transaction_hash = transaction_hash.unwrap_or(H256::zero());

  // set parent hash
  let parent_hash = if block_number > U256::zero() {
    block_by_number(block_number - U256::one()).unwrap().hash
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

  set_block(&block_number, &block);

  (block_number, block_hash)
}
