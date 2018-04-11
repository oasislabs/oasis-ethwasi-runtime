use super::{EthereumRPC, Either, RPCStep, RPCTransaction, RPCBlock, RPCLog, RPCReceipt, RPCTopicFilter, RPCLogFilter, RPCTraceConfig, RPCBreakpointConfig, RPCSourceMapConfig};
use super::filter::*;
use super::serialize::*;
use super::solidity::*;
use error::Error;
use miner::MinerState;

use rlp::{self, UntrustedRlp};
use bigint::{M256, U256, H256, H2048, Address, Gas};
use hexutil::{read_hex, to_hex};
use block::{Block, TotalHeader, Account, Log, Receipt, FromKey, Transaction, UnsignedTransaction, TransactionAction, GlobalSignaturePatch, RlpHash};
use blockchain::chain::HeaderHash;
use sputnikvm::{ValidTransaction, VM, VMStatus, MachineStatus, HeaderParams, SeqTransactionVM, Patch, Memory, AccountChange};
use sputnikvm_stateful::MemoryStateful;
use std::str::FromStr;
use std::collections::HashMap;
use std::rc::Rc;
use sha3::{Keccak256, Digest};

use jsonrpc_macros::Trailing;

pub fn from_block_number<T: Into<Option<String>>>(state: &MinerState, value: T) -> Result<usize, Error> {
    let value: Option<String> = value.into();

    if value == Some("latest".to_string()) || value == Some("pending".to_string()) || value == None {
        Ok(state.block_height())
    } else if value == Some("earliest".to_string()) {
        Ok(0)
    } else {
        let v: u64 = U256::from(read_hex(&value.unwrap())?.as_slice()).into();
        let v = v as usize;
        if v > state.block_height() {
            Err(Error::NotFound)
        } else {
            Ok(v)
        }
    }
}

pub fn to_rpc_log(receipt: &Receipt, index: usize, transaction: &Transaction, block: &Block) -> RPCLog {
    use sha3::{Keccak256, Digest};

    let transaction_hash = H256::from(Keccak256::digest(&rlp::encode(transaction).to_vec()).as_slice());
    let transaction_index = {
        let mut i = 0;
        let mut found = false;
        for transaction in &block.transactions {
            let other_hash = H256::from(Keccak256::digest(&rlp::encode(transaction).to_vec()).as_slice());
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

pub fn to_rpc_receipt(state: &MinerState, receipt: Receipt, transaction: &Transaction, block: &Block) -> Result<RPCReceipt, Error> {
    use sha3::{Keccak256, Digest};

    let transaction_hash = H256::from(Keccak256::digest(&rlp::encode(transaction).to_vec()).as_slice());
    let transaction_index = {
        let mut i = 0;
        let mut found = false;
        for transaction in &block.transactions {
            let other_hash = H256::from(Keccak256::digest(&rlp::encode(transaction).to_vec()).as_slice());
            if transaction_hash == other_hash {
                found = true;
                break;
            }
            i += 1;
        }
        assert!(found);
        i
    };

    let cumulative_gas_used = {
        let mut sum = Gas::zero();

        for i in 0..(transaction_index + 1) {
            let other_hash = H256::from(Keccak256::digest(&rlp::encode(&block.transactions[i]).to_vec()).as_slice());
            sum = sum + state.get_receipt_by_transaction_hash(other_hash)?.used_gas;
        }
        sum
    };

    let contract_address = {
        if transaction.action == TransactionAction::Create {
            Some(transaction.address().unwrap())
        } else {
            None
        }
    };

    Ok(RPCReceipt {
        transaction_hash: Hex(transaction_hash),
        transaction_index: Hex(transaction_index),
        block_hash: Hex(block.header.header_hash()),
        block_number: Hex(block.header.number),
        cumulative_gas_used: Hex(cumulative_gas_used),
        gas_used: Hex(receipt.used_gas),
        contract_address: contract_address.map(|v| Hex(v)),
        logs: {
            let mut ret = Vec::new();
            for i in 0..receipt.logs.len() {
                ret.push(to_rpc_log(&receipt, i, transaction, block));
            }
            ret
        },
        root: Hex(receipt.state_root),
        status: if state.receipt_status(transaction.rlp_hash()) { 1 } else { 0 },
    })
}

pub fn to_rpc_transaction(transaction: Transaction, block: Option<&Block>) -> RPCTransaction {
    use sha3::{Keccak256, Digest};
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
                    let other_hash = H256::from(Keccak256::digest(&rlp::encode(transaction).to_vec()).as_slice());
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

pub fn to_rpc_block(block: Block, total_header: TotalHeader, full_transactions: bool) -> RPCBlock {
    use sha3::{Keccak256, Digest};
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
            Either::Right(block.transactions.iter().map(|t| to_rpc_transaction(t.clone(), Some(&block))).collect())
        } else {
            Either::Left(block.transactions.iter().map(|t| {
                let encoded = rlp::encode(t).to_vec();
                Hex(H256::from(Keccak256::digest(&encoded).as_slice()))
            }).collect())
        },
        uncles: block.ommers.iter().map(|u| Hex(u.header_hash())).collect(),
    }
}

pub fn to_signed_transaction(state: &MinerState, transaction: RPCTransaction, stateful: &MemoryStateful) -> Result<Transaction, Error> {
    let address = match transaction.from {
        Some(val) => val.0,
        None => Address::default(),
    };
    let secret_key = {
        let mut secret_key = None;
        for key in state.accounts() {
            if Address::from_secret_key(&key)? == address {
                secret_key = Some(key);
            }
        }
        match secret_key {
            Some(val) => val,
            None => return Err(Error::NotFound),
        }
    };
    let block = state.get_block_by_number(state.block_height());
    let trie = stateful.state_of(block.header.state_root);

    let account: Option<Account> = trie.get(&address);

    let unsigned = UnsignedTransaction {
        nonce: match transaction.nonce {
            Some(val) => val.0,
            None => {
                account.as_ref().map(|account| account.nonce).unwrap_or(U256::zero())
            }
        },
        gas_price: match transaction.gas_price {
            Some(val) => val.0,
            None => Gas::zero(),
        },
        gas_limit: match transaction.gas {
            Some(val) => val.0,
            None => Gas::from(90000u64),
        },
        action: match transaction.to {
            Some(val) => TransactionAction::Call(val.0),
            None => TransactionAction::Create,
        },
        value: match transaction.value {
            Some(val) => val.0,
            None => U256::zero(),
        },
        input: match transaction.data {
            Some(val) => val.0,
            None => Vec::new(),
        },
    };
    let transaction = unsigned.sign::<GlobalSignaturePatch>(&secret_key);

    Ok(transaction)
}

pub fn to_valid_transaction(state: &MinerState, transaction: RPCTransaction, stateful: &MemoryStateful) -> Result<ValidTransaction, Error> {
    let address = match transaction.from {
        Some(val) => val.0,
        None => Address::default(),
    };

    let block = state.get_block_by_number(state.block_height());
    let trie = stateful.state_of(block.header.state_root);

    let account: Option<Account> = trie.get(&address);

    let valid = ValidTransaction {
        nonce: match transaction.nonce {
            Some(val) => val.0,
            None => {
                account.as_ref().map(|account| account.nonce).unwrap_or(U256::zero())
            }
        },
        gas_price: match transaction.gas_price {
            Some(val) => val.0,
            None => Gas::zero(),
        },
        gas_limit: match transaction.gas {
            Some(val) => val.0,
            None => Gas::from(90000u64),
        },
        action: match transaction.to {
            Some(val) => TransactionAction::Call(val.0),
            None => TransactionAction::Create,
        },
        value: match transaction.value {
            Some(val) => val.0,
            None => U256::zero(),
        },
        input: Rc::new(match transaction.data {
            Some(val) => val.0,
            None => Vec::new(),
        }),
        caller: Some(address),
    };

    Ok(valid)
}

pub fn from_topic_filter(filter: Option<RPCTopicFilter>) -> Result<TopicFilter, Error> {
    Ok(match filter {
        None => TopicFilter::All,
        Some(RPCTopicFilter::Single(s)) => TopicFilter::Or(vec![
            s.0
        ]),
        Some(RPCTopicFilter::Or(ss)) => {
            TopicFilter::Or(ss.into_iter().map(|v| v.0).collect())
        },
    })
}

pub fn from_log_filter(state: &MinerState, filter: RPCLogFilter) -> Result<LogFilter, Error> {
    Ok(LogFilter {
        from_block: from_block_number(state, filter.from_block)?,
        to_block: from_block_number(state, filter.to_block)?,
        address: match filter.address {
            Some(val) => Some(val.0),
            None => None,
        },
        topics: match filter.topics {
            Some(topics) => {
                let mut ret = Vec::new();
                for i in 0..4 {
                    if topics.len() > i {
                        ret.push(from_topic_filter(topics[i].clone())?);
                    } else {
                        ret.push(TopicFilter::All);
                    }
                }
                ret
            },
            None => vec![TopicFilter::All, TopicFilter::All, TopicFilter::All, TopicFilter::All],
        },
    })
}

pub fn replay_transaction<P: Patch>(
    stateful: &MemoryStateful<'static>, transaction: Transaction, block: &Block,
    last_hashes: &[H256], config: &RPCTraceConfig
) -> Result<(Vec<RPCStep>, SeqTransactionVM<P>), Error> {
    let valid = stateful.to_valid::<P>(transaction)?;
    let mut vm = SeqTransactionVM::<P>::new(valid, HeaderParams::from(&block.header));
    let mut steps = Vec::new();
    let mut last_gas = Gas::zero();

    loop {
        match vm.status() {
            VMStatus::ExitedOk | VMStatus::ExitedErr(_) => break,
            VMStatus::ExitedNotSupported(_) => panic!(),
            VMStatus::Running => {
                stateful.step(&mut vm, block.header.number, &last_hashes);
                let gas = vm.used_gas();
                let gas_cost = gas - last_gas;

                last_gas = gas;

                if let Some(machine) = vm.current_machine() {
                    let depth = machine.state().depth;
                    let error = match machine.status() {
                        MachineStatus::ExitedErr(err) => format!("{:?}", err),
                        _ => "".to_string(),
                    };
                    let pc = machine.pc().position();
                    let opcode_pc = machine.pc().opcode_position();
                    let op = machine.pc().code()[pc];
                    let code_hash = H256::from(Keccak256::digest(machine.pc().code()).as_slice());
                    let address = machine.state().context.address;

                    let memory = if config.disable_memory {
                        None
                    } else {
                        let mut ret = Vec::new();
                        for i in 0..machine.state().memory.len() {
                            ret.push(machine.state().memory.read_raw(U256::from(i)));
                        }
                        Some(vec![Bytes(ret)])
                    };
                    let stack = if config.disable_stack {
                        None
                    } else {
                        let mut ret = Vec::new();

                        for i in 0..machine.state().stack.len() {
                            ret.push(Hex(machine.state().stack.peek(i).unwrap()));
                        }
                        Some(ret)
                    };
                    let storage = if config.disable_storage {
                        None
                    } else {
                        let mut for_storage = None;
                        let context_address = machine.state().context.address;

                        for account in machine.state().account_state.accounts() {
                            match account {
                                &AccountChange::Full { address, ref changing_storage, .. } => {
                                    if address == context_address {
                                        for_storage = Some(changing_storage.clone());
                                    }
                                },
                                &AccountChange::Create { address, ref storage, .. } => {
                                    if address == context_address {
                                        for_storage = Some(storage.clone());
                                    }
                                },
                                _ => (),
                            }
                        }

                        let storage = for_storage;
                        let mut ret = HashMap::new();
                        if let Some(storage) = storage {
                            let storage: HashMap<U256, M256> = storage.clone().into();
                            for (key, value) in storage {
                                ret.insert(Hex(key), Hex(value));
                            }
                        }
                        Some(ret)
                    };

                    if let &Some(RPCBreakpointConfig {
                        ref source_map, ref breakpoints
                    }) = &config.breakpoints {
                        if let Some(&RPCSourceMapConfig { ref source_map, ref source_list }) =
                            source_map.get(&Hex(code_hash))
                        {
                            let source_map = parse_source_map(source_map, source_list)?;
                            let source_map = &source_map[opcode_pc];

                            let breakpoints = parse_source(breakpoints)?;
                            if let Some((breakpoint_index, breakpoint)) =
                                source_map.source.find_intersection(&breakpoints)
                            {
                                steps.push(RPCStep {
                                    depth,
                                    error,
                                    gas: Hex(gas),
                                    gas_cost: Hex(gas_cost),
                                    breakpoint_index: Some(breakpoint_index),
                                    breakpoint: Some(format!(
                                        "{}:{}:{}", breakpoint.offset, breakpoint.length,
                                        breakpoint.file_name)),
                                    code_hash: Hex(code_hash),
                                    address: Hex(address),
                                    memory,
                                    op, pc, opcode_pc,
                                    stack,
                                    storage
                                });
                            }
                        }
                    } else {
                        steps.push(RPCStep {
                            depth,
                            error,
                            gas: Hex(gas),
                            gas_cost: Hex(gas_cost),
                            breakpoint_index: None,
                            breakpoint: None,
                            code_hash: Hex(code_hash),
                            address: Hex(address),
                            memory,
                            op, pc, opcode_pc,
                            stack,
                            storage
                        });
                    }
                }
            },
        }
    }

    Ok((steps, vm))
}
