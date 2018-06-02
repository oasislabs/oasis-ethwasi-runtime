use bigint::{H256, U256};
use evm_api::Block;
use sha3::{Digest, Keccak256};
use state::StateDb;

// "mine" a block containing 0 or 1 transactions
// returns block number and hash
pub fn mine_block(transaction_hash: Option<H256>) -> (U256, H256) {
    // get the next block number
    let number = next_block_number();

    // create a block
    let transaction_hash = match transaction_hash {
        Some(val) => val,
        None => H256::new(),
    };

    // set parent hash
    let parent_hash = if number > U256::zero() {
        get_block(number - U256::one()).unwrap().hash
    } else {
        // genesis block
        H256::new()
    };

    // compute a unique block hash
    // WARNING: the value is deterministic and guessable!
    let hash = H256::from(
        Keccak256::digest_str(&format!(
            "{:x} {:x} {:x}",
            number, transaction_hash, parent_hash
        )).as_slice(),
    );

    let block = Block {
        number: number,
        transaction_hash: transaction_hash,
        parent_hash: parent_hash,
        hash: hash,
        transaction: None,
    };

    // store the block
    let state = StateDb::new();
    state.blocks.insert(&number, &block);

    (number, hash)
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

fn next_block_number() -> U256 {
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
