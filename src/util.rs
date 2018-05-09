use bigint::{Address, Gas, H256, U256};
use block::{RlpHash, Transaction, TransactionSignature};
use sputnikvm::{Patch, PreExecutionError, TransactionAction, ValidTransaction};

use std::rc::Rc;
use std::str::FromStr;

use hexutil::{read_hex, ParseHexError};

use evm::{get_balance, get_nonce};
use evm_api::Transaction as EVMTransaction;

pub fn normalize_hex_str(hex: &str) -> String {
    hex.to_lowercase().trim_left_matches("0x").to_string()
}

// WARNING: FOR DEVELOPMENT+TESTING ONLY. DISABLE IN PRODUCTION!
// computes transaction hash from an unsigned web3 sendTransaction
// signature is fake, but unique per account
pub fn unsigned_transaction_hash(transaction: &ValidTransaction) -> H256 {
    // unique per-account fake "signature"
    let signature = TransactionSignature {
        v: 0,
        r: match transaction.caller {
            Some(val) => H256::from(val),
            None => H256::new(),
        },
        s: H256::new(),
    };

    let block_transaction = Transaction {
        nonce: transaction.nonce,
        gas_price: transaction.gas_price,
        gas_limit: transaction.gas_limit,
        action: transaction.action,
        value: transaction.value,
        signature: signature,
        input: Rc::new(transaction.input.clone()).to_vec(),
    };

    block_transaction.rlp_hash()
}

// validates transaction and returns a ValidTransaction on success
pub fn to_valid<P: Patch>(
    transaction: &Transaction,
) -> ::std::result::Result<ValidTransaction, PreExecutionError> {
    // debugging
    println!("*** Validate block transaction");
    println!("Data: {:?}", transaction);

    // check caller signature
    let caller = match transaction.caller() {
        Ok(val) => val,
        Err(_) => return Err(PreExecutionError::InvalidCaller),
    };
    let caller_str = caller.hex();

    // check nonce
    // TODO: what if account doesn't exist? for now returning 0
    let nonce = get_nonce(caller_str.clone());
    if nonce != transaction.nonce {
        return Err(PreExecutionError::InvalidNonce);
    }

    let valid = ValidTransaction {
        caller: Some(caller),
        gas_price: transaction.gas_price,
        gas_limit: transaction.gas_limit,
        action: transaction.action.clone(),
        value: transaction.value,
        input: Rc::new(transaction.input.clone()),
        nonce: nonce,
    };

    // check gas limit
    if valid.gas_limit < valid.intrinsic_gas::<P>() {
        return Err(PreExecutionError::InsufficientGasLimit);
    }

    // check balance
    // TODO: what if account doesn't exist? for now returning 0
    let balance = get_balance(caller_str);

    let gas_limit: U256 = valid.gas_limit.into();
    let gas_price: U256 = valid.gas_price.into();

    let (preclaimed_value, overflowed1) = gas_limit.overflowing_mul(gas_price);
    let (total, overflowed2) = preclaimed_value.overflowing_add(valid.value);
    if overflowed1 || overflowed2 {
        return Err(PreExecutionError::InsufficientBalance);
    }

    if balance < total {
        return Err(PreExecutionError::InsufficientBalance);
    }

    Ok(valid)
}

// constructs a "valid" transaction from an unsigned transaction
// used for eth_call and the non-validating eth_sendTransaction testing interface
pub fn unsigned_to_valid(
    transaction: &EVMTransaction,
) -> ::std::result::Result<ValidTransaction, ParseHexError> {
    let action = if transaction.get_is_call() {
        TransactionAction::Call(Address::from_str(transaction.get_address().clone())?)
    } else {
        TransactionAction::Create
    };

    let caller_str = transaction.get_caller();

    // we're not actually validating, so don't need to verify that nonce matches
    let nonce = if transaction.get_use_nonce() {
        U256::from_str(transaction.get_nonce().clone())?
    } else {
        get_nonce(caller_str.to_string())
    };

    Ok(ValidTransaction {
        caller: Some(Address::from_str(caller_str.clone())?),
        action: action,
        gas_price: Gas::zero(),
        gas_limit: Gas::max_value(),
        value: U256::zero(),
        input: Rc::new(read_hex(transaction.get_input())?),
        nonce: nonce,
    })
}
