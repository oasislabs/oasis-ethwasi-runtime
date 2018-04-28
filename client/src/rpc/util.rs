use super::{Either, RPCStep, RPCTransaction, RPCBlock, RPCLog, RPCReceipt, RPCTopicFilter, RPCLogFilter, RPCTraceConfig, RPCBreakpointConfig, RPCSourceMapConfig};
use super::serialize::*;
use super::solidity::*;
use error::Error;

use rlp::{self, UntrustedRlp};
use bigint::{M256, U256, H256, H2048, Gas};
use hexutil::to_hex;
use block::{Block, TotalHeader, Receipt, Transaction, TransactionAction};
use blockchain::chain::HeaderHash;
use sputnikvm::{VM, VMStatus, MachineStatus, HeaderParams, SeqTransactionVM, Patch, Memory, AccountChange};
use sputnikvm_stateful::MemoryStateful;
use std::collections::HashMap;
use sha3::{Keccak256, Digest};

use evm_api::{Transaction as EVMTransaction};

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
        },
        None => _transaction.set_is_call(false),
    };

    Ok(_transaction)
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
