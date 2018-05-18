use super::{Either, RPCBlock, RPCReceipt, RPCTransaction};
use super::serialize::*;
use error::Error;

use bigint::{Address, Gas, H2048, H256, H64, U256};
use hexutil::{read_hex, to_hex};

use evm_api::{Block, Transaction as EVMTransaction, TransactionRecord};

use std::str::FromStr;

pub fn to_rpc_block(block: &Block, full_transactions: bool) -> Result<RPCBlock, Error> {
    Ok(RPCBlock {
        number: Hex(U256::from_str(block.get_number())?),
        hash: Hex(H256::from_str(block.get_hash())?),
        parent_hash: Hex(H256::from_str(block.get_parent_hash())?),
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
            Either::Right(match to_rpc_transaction(block.get_transaction()) {
                Ok(val) => vec![val],
                Err(_) => Vec::new(),
            })
        } else {
            Either::Left(match H256::from_str(block.get_transaction_hash()) {
                Ok(val) => vec![Hex(val)],
                Err(_) => Vec::new(),
            })
        },
        uncles: Vec::new(),
    })
}

pub fn to_rpc_receipt(record: &TransactionRecord) -> Result<RPCReceipt, Error> {
    Ok(RPCReceipt {
        transaction_hash: Hex(H256::from_str(record.get_hash())?),
        transaction_index: Hex(record.get_index() as usize),
        block_hash: Hex(H256::from_str(record.get_block_hash())?),
        block_number: Hex(U256::from_str(record.get_block_number())?),
        cumulative_gas_used: Hex(Gas::from_str(record.get_cumulative_gas_used())?),
        gas_used: Hex(Gas::from_str(record.get_gas_used())?),
        contract_address: if record.get_is_create() { Some(Hex(Address::from_str(record.get_contract_address())?)) } else { None },
        // TODO: logs
        logs: Vec::new(),
        root: Hex(H256::new()),
        status: if record.get_status() { 1 } else { 0 },
    })
}

pub fn to_rpc_transaction(record: &TransactionRecord) -> Result<RPCTransaction, Error> {
    Ok(RPCTransaction {
        from: Some(Hex(Address::from_str(record.get_from())?)),
        to: if record.get_is_create() {
            None
        } else {
            Some(Hex(Address::from_str(record.get_to())?))
        },
        gas: Some(Hex(Gas::from_str(record.get_gas_provided())?)),
        gas_price: Some(Hex(Gas::from_str(record.get_gas_price())?)),
        value: Some(Hex(U256::from_str(record.get_value())?)),
        data: Some(Bytes(read_hex(record.get_input())?)),
        nonce: Some(Hex(U256::from_str(record.get_nonce())?)),

        hash: Some(Hex(H256::from_str(record.get_hash())?)),
        block_hash: Some(Hex(H256::from_str(record.get_block_hash())?)),
        block_number: Some(Hex(U256::from_str(record.get_block_number())?)),
        transaction_index: Some(Hex(record.get_index() as usize)),
    })
}

pub fn to_evm_transaction(transaction: RPCTransaction) -> Result<EVMTransaction, Error> {
    let mut _transaction = EVMTransaction::new();

    if let Some(val) = transaction.from {
        _transaction.set_caller(val.0.hex());
    }

    if let Some(val) = transaction.data.clone() {
        _transaction.set_input(to_hex(&val.0));
    }

    match transaction.nonce {
        Some(val) => {
            _transaction.set_use_nonce(true);
            _transaction.set_nonce(format!("{}", val.0));
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

    if let Some(val) = transaction.value {
        _transaction.set_value(format!("{:x}", val.0));
    }

    Ok(_transaction)
}
