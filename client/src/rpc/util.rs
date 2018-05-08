use super::{Either, RPCBlock, RPCLog, RPCReceipt, RPCTransaction};
use super::serialize::*;
use error::Error;

use bigint::{Gas, H2048, H256, M256, U256};
use block::{Block, Receipt, TotalHeader, Transaction, TransactionAction};
use blockchain::chain::HeaderHash;
use hexutil::to_hex;
use rlp::{self};

use evm_api::Transaction as EVMTransaction;

pub fn to_rpc_log(
    receipt: &Receipt,
    index: usize,
    transaction: &Transaction,
    block: &Block,
) -> RPCLog {
    use sha3::{Digest, Keccak256};

    let transaction_hash =
        H256::from(Keccak256::digest(&rlp::encode(transaction).to_vec()).as_slice());
    let transaction_index = {
        let mut i = 0;
        let mut found = false;
        for transaction in &block.transactions {
            let other_hash =
                H256::from(Keccak256::digest(&rlp::encode(transaction).to_vec()).as_slice());
            if transaction_hash == other_hash {
                found = true;
                break;
            }
            i += 1;
        }
        assert!(found);
        i
    };

    RPCLog {
        removed: false,
        log_index: Hex(index),
        transaction_index: Hex(transaction_index),
        transaction_hash: Hex(transaction_hash),
        block_hash: Hex(block.header.header_hash()),
        block_number: Hex(block.header.number),
        data: Bytes(receipt.logs[index].data.clone()),
        topics: receipt.logs[index].topics.iter().map(|t| Hex(*t)).collect(),
    }
}

pub fn to_rpc_transaction(transaction: Transaction, block: Option<&Block>) -> RPCTransaction {
    use sha3::{Digest, Keccak256};
    let hash = H256::from(Keccak256::digest(&rlp::encode(&transaction).to_vec()).as_slice());

    RPCTransaction {
        from: Some(Hex(transaction.caller().unwrap())),
        to: match transaction.action {
            TransactionAction::Call(address) => Some(Hex(address)),
            TransactionAction::Create => None,
        },
        gas: Some(Hex(transaction.gas_limit)),
        gas_price: Some(Hex(transaction.gas_price)),
        value: Some(Hex(transaction.value)),
        data: Some(Bytes(transaction.input)),
        nonce: Some(Hex(transaction.nonce)),

        hash: Some(Hex(hash)),
        block_hash: block.map(|b| Hex(b.header.header_hash())),
        block_number: block.map(|b| Hex(b.header.number)),
        transaction_index: {
            if block.is_some() {
                let block = block.unwrap();
                let mut i = 0;
                let mut found = false;
                for transaction in &block.transactions {
                    let other_hash = H256::from(
                        Keccak256::digest(&rlp::encode(transaction).to_vec()).as_slice(),
                    );
                    if hash == other_hash {
                        found = true;
                        break;
                    }
                    i += 1;
                }
                if found {
                    Some(Hex(i))
                } else {
                    None
                }
            } else {
                None
            }
        },
    }
}

pub fn to_evm_transaction(transaction: RPCTransaction) -> Result<EVMTransaction, Error> {
    let mut _transaction = EVMTransaction::new();

    match transaction.from {
        Some(val) => _transaction.set_caller(val.0.hex()),
        None => {}
    };

    match transaction.data.clone() {
        Some(val) => _transaction.set_input(to_hex(&val.0)),
        None => {}
    };

    match transaction.nonce {
        Some(val) => {
            _transaction.set_use_nonce(true);
            _transaction.set_nonce(format!("{:x}", val.0));
        }
        None => _transaction.set_use_nonce(false),
    };

    match transaction.to {
        Some(val) => {
            _transaction.set_is_call(true);
            _transaction.set_address(val.0.hex());
        }
        None => _transaction.set_is_call(false),
    };

    Ok(_transaction)
}

pub fn to_rpc_block(block: Block, total_header: TotalHeader, full_transactions: bool) -> RPCBlock {
    use sha3::{Digest, Keccak256};
    let logs_bloom: H2048 = block.header.logs_bloom.clone().into();

    RPCBlock {
        number: Hex(block.header.number),
        hash: Hex(block.header.header_hash()),
        parent_hash: Hex(block.header.parent_hash),
        nonce: Hex(block.header.nonce),
        sha3_uncles: Hex(block.header.ommers_hash),
        logs_bloom: Hex(logs_bloom),
        transactions_root: Hex(block.header.transactions_root),
        state_root: Hex(block.header.state_root),
        receipts_root: Hex(block.header.receipts_root),
        miner: Hex(block.header.beneficiary),
        difficulty: Hex(block.header.difficulty),
        total_difficulty: Hex(total_header.total_difficulty()),

        // TODO: change this to the correct one after the Typhoon is over...
        extra_data: Bytes(rlp::encode(&block.header.extra_data).to_vec()),

        size: Hex(rlp::encode(&block.header).to_vec().len()),
        gas_limit: Hex(block.header.gas_limit),
        gas_used: Hex(block.header.gas_used),
        timestamp: Hex(block.header.timestamp),
        transactions: if full_transactions {
            Either::Right(
                block
                    .transactions
                    .iter()
                    .map(|t| to_rpc_transaction(t.clone(), Some(&block)))
                    .collect(),
            )
        } else {
            Either::Left(
                block
                    .transactions
                    .iter()
                    .map(|t| {
                        let encoded = rlp::encode(t).to_vec();
                        Hex(H256::from(Keccak256::digest(&encoded).as_slice()))
                    })
                    .collect(),
            )
        },
        uncles: block.ommers.iter().map(|u| Hex(u.header_hash())).collect(),
    }
}

