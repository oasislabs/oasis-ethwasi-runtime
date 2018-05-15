use bigint::{H256, U256};
use evm::StateDb;
use evm_api::Block;
use sha3::{Digest, Keccak256};
use std::str::FromStr;

// "mine" a block containing a single transaction
// returns block number and hash
pub fn mine_block(transaction_hash: H256) -> (U256, H256) {
    let number = next_block_number();

    // create a new block
    let mut block = Block::new();
    block.set_number(format!("{:x}", number));
    block
        .transaction_hashes
        .push(format!("{:x}", transaction_hash));

    // set parent hash
    block.set_parent_hash(match get_block(number - U256::one()) {
        Some(val) => val.get_hash().to_string(),
        None => format!("{:x}", H256::new()),
    });

    // compute a unique block hash
    // WARNING: the value is deterministic and guessable!
    let hash = H256::from(Keccak256::digest_str(&format!("{:?}", block)).as_slice());
    block.set_hash(format!("{:x}", hash));

    // store the block
    let state = StateDb::new();
    state.blocks.insert(&format!("{:x}", number), &block);

    println!("Mining block {:?}", block);

    (number, hash)
}

pub fn get_block(number: U256) -> Option<Block> {
    let state = StateDb::new();
    state.blocks.get(&format!("{:x}", number))
}

pub fn get_latest_block_number() -> U256 {
    let state = StateDb::new();
    let latest = match state.latest_block_number.get() {
        Some(val) => U256::from_str(&val).unwrap(),
        None => U256::zero(),
    };
    latest
}

fn next_block_number() -> U256 {
    let state = StateDb::new();

    // get latest block number
    let latest = match state.latest_block_number.get() {
        Some(val) => U256::from_str(&val).unwrap(),
        None => U256::zero(),
    };

    // increment block number and store new value
    let next = latest + U256::one();
    state.latest_block_number.insert(&format!("{:x}", next));
    next
}
