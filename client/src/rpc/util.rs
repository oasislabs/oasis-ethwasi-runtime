use super::serialize::*;
use super::{Either, RPCBlock, RPCLog, RPCReceipt, RPCTransaction};
use error::Error;

use bigint::{Address, Gas, H2048, H256, H64, U256};
use hexutil::{read_hex, to_hex};

use evm_api::{Block, Transaction as EVMTransaction, TransactionRecord};

use std::str::FromStr;

pub fn to_rpc_log(record: &TransactionRecord, index: usize) -> RPCLog {
    RPCLog {
        removed: false,
        log_index: Hex(index),
        transaction_index: Hex(0),
        transaction_hash: Hex(record.hash),
        block_hash: Hex(record.block_hash),
        block_number: Hex(record.block_number),
        data: Bytes(record.logs[index].data.clone()),
        topics: record.logs[index].topics.iter().map(|t| Hex(*t)).collect(),
    }
}

pub fn to_rpc_block(block: Block, full_transactions: bool) -> Result<RPCBlock, Error> {
    Ok(RPCBlock {
        number: Hex(block.number),
        hash: Hex(block.hash),
        parent_hash: Hex(block.parent_hash),
        nonce: Hex(H64::new()),
        sha3_uncles: Hex(H256::new()),
        logs_bloom: Hex(H2048::new()),
        transactions_root: Hex(H256::new()),
        state_root: Hex(H256::new()),
        receipts_root: Hex(H256::new()),
        miner: Hex(Address::default()),
        difficulty: Hex(U256::zero()),
        total_difficulty: Hex(U256::zero()),
        extra_data: Bytes(Vec::new()),

        size: Hex(0),
        // FIXME: gas_limits that are too high overflow metamask, so pick an arbitrary not-too-large number
        gas_limit: Hex(Gas::from_str("0x10000000000000").unwrap()),
        gas_used: Hex(Gas::zero()),
        timestamp: Hex(0),
        transactions: if full_transactions {
            Either::Right(match block.transaction {
                Some(transaction) => match to_rpc_transaction(&transaction) {
                    Ok(val) => vec![val],
                    Err(_) => Vec::new(),
                },
                None => Vec::new(),
            })
        } else {
            Either::Left(vec![Hex(block.transaction_hash)])
        },
        uncles: Vec::new(),
    })
}

pub fn to_rpc_receipt(record: &TransactionRecord) -> Result<RPCReceipt, Error> {
    Ok(RPCReceipt {
        transaction_hash: Hex(record.hash),
        transaction_index: Hex(record.index as usize),
        block_hash: Hex(record.block_hash),
        block_number: Hex(record.block_number),
        cumulative_gas_used: Hex(record.cumulative_gas_used),
        gas_used: Hex(record.gas_used),
        contract_address: if record.is_create {
            match record.contract_address {
                Some(address) => Some(Hex(address)),
                None => None,
            }
        } else {
            None
        },
        logs: {
            let mut ret = Vec::new();
            for i in 0..record.logs.len() {
                ret.push(to_rpc_log(&record, i));
            }
            ret
        },
        root: Hex(H256::new()),
        status: if record.status { 1 } else { 0 },
    })
}

pub fn to_rpc_transaction(record: &TransactionRecord) -> Result<RPCTransaction, Error> {
    Ok(RPCTransaction {
        from: match record.from {
            Some(address) => Some(Hex(address)),
            None => None,
        },
        to: if record.is_create {
            None
        } else {
            match record.to {
                Some(address) => Some(Hex(address)),
                None => None,
            }
        },
        gas: Some(Hex(record.gas_provided)),
        gas_price: Some(Hex(record.gas_price)),
        value: Some(Hex(record.value)),
        data: Some(Bytes(read_hex(&record.input)?)),
        nonce: Some(Hex(record.nonce)),

        hash: Some(Hex(record.hash)),
        block_hash: Some(Hex(record.block_hash)),
        block_number: Some(Hex(record.block_number)),
        transaction_index: Some(Hex(record.index as usize)),
    })
}

pub fn to_evm_transaction(transaction: RPCTransaction) -> Result<EVMTransaction, Error> {
    let _transaction = EVMTransaction {
        caller: match transaction.from {
            Some(val) => Some(val.0),
            None => None,
        },
        input: match transaction.data.clone() {
            Some(val) => to_hex(&val.0),
            None => String::new(),
        },
        nonce: match transaction.nonce {
            Some(val) => Some(val.0),
            None => None,
        },
        is_call: transaction.to.is_some(),
        address: match transaction.to {
            Some(val) => Some(val.0),
            None => None,
        },
        value: match transaction.value {
            Some(val) => Some(val.0),
            None => None,
        },
    };

    Ok(_transaction)
}
