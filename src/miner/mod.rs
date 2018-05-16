use bigint::{H256, U256};
use evm::StateDb;
use evm_api::Block;
use sha3::{Digest, Keccak256};
use std::str::FromStr;

// "mine" a block containing 0 or 1 transactions
// returns block number and hash
pub fn mine_block(transaction_hash: Option<H256>) -> (U256, H256) {
    let number = next_block_number();

    // create a new block
    let mut block = Block::new();
    block.set_number(format!("{:x}", number));
    if let Some(val) = transaction_hash {
        block.set_transaction_hash(format!("{:x}", val));
    }

    let parent_hash = if number > U256::zero() {
        get_block(number - U256::one())
            .unwrap()
            .get_hash()
            .to_string()
    } else {
        // genesis block
        format!("{:x}", H256::new())
    };
    block.set_parent_hash(parent_hash);

    // compute a unique block hash
    // WARNING: the value is deterministic and guessable!
    let hash = H256::from(Keccak256::digest_str(&format!("{:?}", block)).as_slice());
    block.set_hash(format!("{:x}", hash));

    // store the block
    let state = StateDb::new();
    state.blocks.insert(&format!("{:x}", number), &block);

    println!("Mining block number {:?} {:?}", number, block);

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

    let next = if (state.latest_block_number.is_present()) {
        U256::from_str(&state.latest_block_number.get().unwrap()).unwrap() + U256::one()
    } else {
        // genesis block
        U256::zero()
    };

    //  store new value
    state.latest_block_number.insert(&format!("{:x}", next));
    next
}
