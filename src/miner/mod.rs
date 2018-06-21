use bigint::{H256, U256};
use evm_api::Block;
use sha3::{Digest, Keccak256};
use state::StateDb;

pub struct Miner {
    db: StateDb,
}

impl Miner {
    fn new() -> Self {
        Miner { db: StateDb::new() }
    }

    pub fn instance() -> Miner {
        Miner::new()
    }

    // "mine" a block containing 0 or 1 transactions
    // returns block number and hash
    pub fn mine_block(&self, transaction_hash: Option<H256>) -> (U256, H256) {
        // get the next block number
        let number = self.next_block_number();

        // create a block
        let transaction_hash = match transaction_hash {
            Some(val) => val,
            None => H256::new(),
        };

        // set parent hash
        let parent_hash = if number > U256::zero() {
            self.block_by_number(number - U256::one()).unwrap().hash
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
        self.db.blocks.insert(&number, &block);
        (number, hash)
    }

    pub fn block_by_number(&self, number: U256) -> Option<Block> {
        self.db.blocks.get(&number)
    }

    pub fn block_by_hash(&self, hash: H256) -> Option<Block> {
        match self.db.block_hashes.get(&hash) {
            Some(number) => self.block_by_number(number),
            None => None,
        }
    }

    pub fn get_block_hash(&self, number: U256) -> Option<H256> {
        match self.db.blocks.get(&number) {
            Some(block) => Some(block.hash),
            None => None,
        }
    }

    pub fn get_latest_block_number(&self) -> U256 {
        self.db.latest_block_number.get().unwrap_or(U256::zero())
    }

    fn next_block_number(&self) -> U256 {
        let next = if self.db.latest_block_number.is_present() {
            self.db.latest_block_number.get().unwrap() + U256::one()
        } else {
            // genesis block
            U256::zero()
        };

        // store new value
        self.db.latest_block_number.insert(&next);
        next
    }
}
