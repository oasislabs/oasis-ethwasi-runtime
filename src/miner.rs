use ethcore::{
  block::Block,
  header::Header,
  types::{receipt::Receipt, BlockNumber},
};
use ethereum_types::{H256, U256};
use sha3::{Digest, Keccak256};

use state::{add_block, block_by_number, get_latest_block_number};

/// "mine" a block containing 0 or 1 transactions.
/// Returns block number and hash.
pub fn mine_block(transaction_hash: Option<H256>, state_root: H256) -> (BlockNumber, H256) {
    // get the next block number
    let block_number = if transaction_hash.is_some() {
        get_latest_block_number() + 1
    } else {
        0
    };

    // create a block
    let transaction_hash = transaction_hash.unwrap_or(H256::zero());

    // set parent hash
    let parent_hash = if block_number > 0 {
        block_by_number(block_number - 1).unwrap().header.hash()
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

    // TODO: is setting transactions_root correct?
    let block = Block {
        header: {
            let mut header = Header::new();
            header.set_number(block_number);
            header.set_transactions_root(transaction_hash);
            header.set_parent_hash(parent_hash);
            header.set_state_root(state_root);
            header
        },
        transactions: vec![],
        uncles: Vec::new(),
    };

    set_block(&block_number, &block);

    (block_number, block_hash)
}
