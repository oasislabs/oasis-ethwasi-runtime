use super::{Either, RPCBlock, RPCLog, RPCReceipt, RPCTransaction};
use super::serialize::*;
use error::Error;

use bigint::{Address, Gas, H2048, H256, M256, U256};
use block::{Block, TotalHeader, Transaction, TransactionAction};
use blockchain::chain::HeaderHash;
use hexutil::to_hex;
use rlp;

use evm_api::{Receipt, Transaction as EVMTransaction};

use std::str::FromStr;

pub fn to_rpc_receipt(
    receipt: &Receipt,
) -> Result<RPCReceipt, Error> {
    Ok(RPCReceipt {
        transaction_hash: Hex(H256::from_str(receipt.get_hash()).unwrap()),
        transaction_index: Hex(receipt.get_index() as usize),
        // TODO: block hash
        block_hash: Hex(H256::new()),
        block_number: Hex(U256::from_str(receipt.get_block_number()).unwrap()),
        cumulative_gas_used: Hex(Gas::from_str(receipt.get_cumulative_gas_used()).unwrap()),
        gas_used: Hex(Gas::from_str(receipt.get_gas_used()).unwrap()),
        contract_address: Some(Hex(Address::from_str(receipt.get_contract_address()).unwrap())),
        // TODO: logs
        logs: Vec::new(),
        root: Hex(H256::new()),
        status: if receipt.get_status() {
            1
        } else {
            0
        },
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

