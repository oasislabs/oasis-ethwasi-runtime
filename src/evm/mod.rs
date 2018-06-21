extern crate alloc;
extern crate bigint;
extern crate hexutil;
extern crate sha3;
extern crate sputnikvm;

use bigint::{Address, Gas, H256, Sign, U256};
use hexutil::read_hex;
use miner::Miner;
use sputnikvm::{AccountChange, AccountCommitment, HeaderParams, Patch, RequireError,
                SeqTransactionVM, ValidTransaction, VM};
use state::EthState;
use std::rc::Rc;

pub mod patch;

fn handle_fire<P: Patch>(vm: &mut SeqTransactionVM<P>) {
    let state = EthState::instance();
    loop {
        match vm.fire() {
            Ok(()) => break,
            Err(RequireError::Account(address)) => {
                trace!("> Require Account: {:x}", address);
                let commit = match state.get_account_state(address) {
                    Some(account) => {
                        trace!("  -> Found account");
                        AccountCommitment::Full {
                            nonce: account.nonce,
                            address: address,
                            balance: account.balance,
                            code: Rc::new(read_hex(&account.code).unwrap()),
                        }
                    }
                    None => {
                        trace!("  -> Nonexistent");
                        AccountCommitment::Nonexist(address)
                    }
                };
                vm.commit_account(commit).unwrap();
            }
            Err(RequireError::AccountStorage(address, index)) => {
                trace!("> Require Account Storage: {:x} @ {:x}", address, index);
                let value = state.get_account_storage(address, index);
                trace!("  -> {:?}", value);
                vm.commit_account(AccountCommitment::Storage {
                    address: address,
                    index: index,
                    value: value,
                }).unwrap();
            }
            Err(RequireError::AccountCode(address)) => {
                trace!("> Require Account Code: {:x}", address);
                let commit = match state.get_account_state(address) {
                    Some(account) => {
                        trace!("  -> Found code");
                        AccountCommitment::Code {
                            address: address,
                            code: Rc::new(read_hex(&account.code).unwrap()),
                        }
                    }
                    None => {
                        trace!("  -> Nonexistent");
                        AccountCommitment::Nonexist(address)
                    }
                };
                vm.commit_account(commit).unwrap();
            }
            Err(RequireError::Blockhash(number)) => {
                trace!("> Require Blockhash @ {:x}", number);
                // ethereum returns actual values only for the most recent 256 blocks, otherwise 0
                let miner = Miner::instance();
                let latest = miner.get_latest_block_number();
                let hash = if number <= latest && latest - number < U256::from(256) {
                    miner.get_block_hash(number).unwrap_or(H256::zero())
                } else {
                    H256::zero()
                };
                trace!("  -> {:?}", hash);
                vm.commit_blockhash(number, hash).unwrap();
            }
        }
    }
}

pub fn update_state_from_vm<P: Patch>(vm: &SeqTransactionVM<P>) {
    let state = EthState::instance();
    for account in vm.accounts() {
        match account {
            &AccountChange::Create {
                nonce,
                address,
                balance,
                ref storage,
                ref code,
            } => {
                state.update_account_state(nonce, address, balance, code);
                state.update_account_storage(address, storage);
            }
            &AccountChange::Full {
                nonce,
                address,
                balance,
                ref changing_storage,
                ref code,
            } => {
                state.update_account_state(nonce, address, balance, code);
                state.update_account_storage(address, changing_storage);
            }
            &AccountChange::IncreaseBalance(address, amount) => {
                state.update_account_balance::<P>(&address, amount, Sign::Plus);
            }
            &AccountChange::Nonexist(_address) => {}
        }
    }
}

pub fn fire_transaction<P: Patch>(
    transaction: &ValidTransaction,
    block_number: U256,
) -> SeqTransactionVM<P> {
    let block_header = HeaderParams {
        // TODO: mining reward. currently gas fees are credited to address 0
        beneficiary: Address::default(),
        timestamp: 0,
        number: block_number,
        difficulty: U256::zero(),
        gas_limit: Gas::zero(),
    };

    let mut vm = SeqTransactionVM::new(transaction.clone(), block_header.clone());
    handle_fire(&mut vm);

    trace!("    VM returned: {:?}", vm.status());
    trace!("    VM out: {:?}", vm.out());

    for account in vm.accounts() {
        trace!("        {:?}", account);
    }

    vm
}
