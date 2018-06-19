use bigint::{Address, Gas, H256, U256};
use block::{RlpHash, Transaction, TransactionSignature};
use ekiden_core::error::{Error, Result};
use evm_api::{FilteredLog, LogFilter, TopicFilter, Transaction as EVMTransaction};
use hexutil::{read_hex, ParseHexError};
use miner::Miner;
use sputnikvm::{Log, Patch, PreExecutionError, TransactionAction, ValidTransaction};
use state::EthState;
use std::rc::Rc;

// validates transaction and returns a ValidTransaction on success
pub fn to_valid<P: Patch>(
    transaction: &Transaction,
) -> ::std::result::Result<ValidTransaction, PreExecutionError> {
    debug!("*** Validate block transaction");
    debug!("Data: {:?}", transaction);

    // check caller signature
    let caller = match transaction.caller() {
        Ok(val) => val,
        Err(_) => return Err(PreExecutionError::InvalidCaller),
    };

    let state = EthState::instance();

    // check nonce, always pass in benchmark mode
    let nonce = state.get_account_nonce(&caller);
    if nonce != transaction.nonce {
        if cfg!(feature = "benchmark") {
            debug!("Continuing despite invalid nonce");
        } else {
            return Err(PreExecutionError::InvalidNonce);
        }
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
    let balance = state.get_account_balance(&caller);

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
#[cfg(any(debug_assertions, feature = "benchmark"))]
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
            None => EthState::instance().get_account_nonce(&address),
        },
        None => U256::zero(),
    };

    Ok(ValidTransaction {
        caller: Some(transaction.caller.unwrap_or(Address::default())),
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

fn check_log_topic(log: &Log, index: usize, filter: &TopicFilter) -> bool {
    match filter {
        &TopicFilter::All => true,
        &TopicFilter::Or(ref hashes) => {
            if log.topics.len() >= index {
                false
            } else {
                let mut matched = false;
                for hash in hashes {
                    if hash == &log.topics[index] {
                        matched = true;
                    }
                }
                matched
            }
        }
    }
}

pub fn parse_block_number(value: &Option<String>, latest_block_number: &U256) -> Result<U256> {
    if value == &Some("latest".to_string()) || value == &Some("pending".to_string())
        || value == &None
    {
        Ok(latest_block_number.clone())
    } else if value == &Some("earliest".to_string()) {
        Ok(U256::zero())
    } else {
        match read_hex(&value.clone().unwrap()) {
            Ok(val) => Ok(U256::from(val.as_slice())),
            Err(err) => return Err(Error::new(format!("{:?}", err))),
        }
    }
}

pub fn get_logs_from_filter(filter: &LogFilter) -> Result<Vec<FilteredLog>> {
    let miner = Miner::instance();
    let latest_block_number = miner.get_latest_block_number();
    let from_block = parse_block_number(&filter.from_block, &latest_block_number)?;
    let to_block =
        latest_block_number.min(parse_block_number(&filter.to_block, &latest_block_number)?);
    let state = EthState::instance();

    if from_block > to_block {
        return Err(Error::new(format!("{:?}", "Invalid block range")));
    }

    let mut current_block_number = from_block;
    let mut ret = Vec::new();

    while current_block_number <= to_block {
        let block = match miner.block_by_number(current_block_number) {
            Some(block) => block,
            None => break,
        };

        match state.get_transaction_record(&block.transaction_hash) {
            Some(record) => {
                for i in 0..record.logs.len() {
                    let log = &record.logs[i];

                    let passes_filter = filter.addresses.contains(&log.address)
                        && check_log_topic(log, 0, &filter.topics[0])
                        && check_log_topic(log, 1, &filter.topics[1])
                        && check_log_topic(log, 2, &filter.topics[2])
                        && check_log_topic(log, 3, &filter.topics[3]);

                    if passes_filter {
                        ret.push(FilteredLog {
                            removed: false,
                            log_index: i,
                            transaction_index: 0,
                            transaction_hash: block.transaction_hash,
                            block_hash: block.hash,
                            block_number: block.number,
                            data: record.logs[i].data.clone(),
                            topics: record.logs[i].topics.clone(),
                        });
                    }
                }
            }
            None => {}
        }

        current_block_number = current_block_number + U256::one();
    }

    return Ok(ret);
}
