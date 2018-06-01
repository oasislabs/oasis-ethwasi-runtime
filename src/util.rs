use bigint::{Gas, H256, U256};
use block::{RlpHash, Transaction, TransactionSignature};
use evm_api::Transaction as EVMTransaction;
use hexutil::{read_hex, ParseHexError};
use sputnikvm::{Patch, PreExecutionError, TransactionAction, ValidTransaction};
use state::{get_account_balance, get_account_nonce};
use std::rc::Rc;

// validates transaction and returns a ValidTransaction on success
pub fn to_valid<P: Patch>(
    transaction: &Transaction,
) -> ::std::result::Result<ValidTransaction, PreExecutionError> {
    // debugging
    debug!("*** Validate block transaction");
    debug!("Data: {:?}", transaction);

    // check caller signature
    let caller = match transaction.caller() {
        Ok(val) => val,
        Err(_) => return Err(PreExecutionError::InvalidCaller),
    };

    // check nonce
    // TODO: what if account doesn't exist? for now returning 0
    let nonce = get_account_nonce(&caller);
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
    let balance = get_account_balance(&caller);

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

// for debugging and testing: computes transaction hash from an unsigned web3 sendTransaction
// signature is fake, but unique per account
#[cfg(debug_assertions)]
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

// constructs a "valid" transaction from an unsigned transaction
// used for eth_call and the non-validating eth_sendTransaction testing interface
pub fn unsigned_to_valid(
    transaction: &EVMTransaction,
) -> ::std::result::Result<ValidTransaction, ParseHexError> {
    let action = if transaction.is_call {
        match transaction.address {
            Some(address) => TransactionAction::Call(address),
            None => return Err(ParseHexError::Other),
        }
    } else {
        TransactionAction::Create
    };

    // we're not actually validating, so don't need to verify that nonce matches
    let nonce = match transaction.caller {
        // Request specified a caller. Look up the nonce for this address if not defined in the transaction.
        Some(address) => match transaction.nonce {
            Some(nonce) => nonce,
            None => get_account_nonce(&address),
        },
        None => U256::zero(),
    };

    Ok(ValidTransaction {
        caller: transaction.caller,
        action: action,
        gas_price: Gas::zero(),
        gas_limit: Gas::max_value(),
        value: match transaction.value {
            Some(value) => value,
            None => U256::zero(),
        },
        input: Rc::new(read_hex(&transaction.input)?),
        nonce: nonce,
    })
}
