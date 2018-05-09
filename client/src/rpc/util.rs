use super::{Either, RPCBlock, RPCLog, RPCReceipt, RPCTransaction};
use super::serialize::*;
use error::Error;

use bigint::{Address, Gas, H2048, H256, M256, U256};
use block::{Block, TotalHeader, Transaction, TransactionAction};
use blockchain::chain::HeaderHash;
use hexutil::{read_hex, to_hex};
use rlp;

use evm_api::{Transaction as EVMTransaction, TransactionRecord};

use std::str::FromStr;

pub fn to_rpc_receipt(record: &TransactionRecord) -> Result<RPCReceipt, Error> {
    Ok(RPCReceipt {
        transaction_hash: Hex(H256::from_str(record.get_hash())?),
        transaction_index: Hex(record.get_index() as usize),
        // TODO: block hash
        block_hash: Hex(H256::new()),
        block_number: Hex(U256::from_str(record.get_block_number())?),
        cumulative_gas_used: Hex(Gas::from_str(record.get_cumulative_gas_used())?),
        gas_used: Hex(Gas::from_str(record.get_gas_used())?),
        contract_address: Some(Hex(Address::from_str(record.get_contract_address())?)),
        // TODO: logs
        logs: Vec::new(),
        root: Hex(H256::new()),
        status: if record.get_status() { 1 } else { 0 },
    })
}

pub fn to_rpc_transaction(record: &TransactionRecord) -> Result<RPCTransaction, Error> {
    Ok(RPCTransaction {
        from: Some(Hex(Address::from_str(record.get_from())?)),
        to: {
            if record.get_is_create() {
                None
            } else {
                Some(Hex(Address::from_str(record.get_to())?))
            }
        },
        gas: Some(Hex(Gas::from_str(record.get_gas_provided())?)),
        gas_price: Some(Hex(Gas::from_str(record.get_gas_price())?)),
        value: Some(Hex(U256::from_str(record.get_value())?)),
        data: Some(Bytes(read_hex(record.get_input())?)),
        nonce: Some(Hex(U256::from_str(record.get_nonce())?)),

        hash: Some(Hex(H256::from_str(record.get_hash())?)),
        // TODO: block hash
        block_hash: Some(Hex(H256::new())),
        block_number: Some(Hex(U256::from_str(record.get_block_number())?)),
        transaction_index: Some(Hex(record.get_index() as usize)),
    })
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

    Ok(_transaction)
}
