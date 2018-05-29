extern crate alloc;
extern crate bigint;
extern crate hexutil;
extern crate sha3;
extern crate sputnikvm;

use ekiden_trusted::db::database_schema;

use std::collections::HashMap;

use bigint::{Address, Gas, H256, M256, Sign, U256};

use evm_api::{AccountState, Block, TransactionRecord};
use hexutil::{read_hex, to_hex};

use sputnikvm::{AccountChange, AccountCommitment, AccountPatch, HeaderParams, Patch, RequireError,
                SeqTransactionVM, Storage, TransactionAction, VMStatus, ValidTransaction, VM};
use std::str::FromStr;

use std::rc::Rc;

pub mod patch;

// Create database schema.
database_schema! {
    pub struct StateDb {
        pub genesis_initialized: bool,
        pub accounts: Map<Address, AccountState>,
        pub transactions: Map<H256, TransactionRecord>,
        pub latest_block_number: U256,
        pub blocks: Map<U256, Block>,
    }
}

fn handle_fire<P: Patch>(vm: &mut SeqTransactionVM<P>, state: &StateDb) {
    loop {
        match vm.fire() {
            Ok(()) => break,
            Err(RequireError::Account(address)) => {
                trace!("> Require Account: {:x}", address);
                let commit = match state.accounts.get(&address) {
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
                let value = match state.accounts.get(&address).unwrap().storage.get(&index) {
                    Some(b) => M256(b.clone()),
                    None => M256::zero(),
                };

                trace!("  -> {:?}", value);
                vm.commit_account(AccountCommitment::Storage {
                    address: address,
                    index: index,
                    value: value,
                }).unwrap();
            }
            Err(RequireError::AccountCode(address)) => {
                trace!("> Require Account Code: {:x}", address);
                let commit = match state.accounts.get(&address) {
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
                // TODO: maintain block state (including blockhash)
                let result = match number.as_u32() {
                    4976641 => H256::from_str(
                        "0x4f5bf1c9fc97e2c17a34859bb885a67519c19e2a0d9176da45fcfee758fadf82",
                    ).unwrap(),
                    _ => panic!("VM requested blockhash of unknown block: {}", number),
                };

                vm.commit_blockhash(number, result).unwrap();
            }
        }
    }
}

// TODO: currently returns 0 for nonexistent accounts
//       specified behavior is different for more recent patches
pub fn get_nonce(address: &Address) -> U256 {
    let state = StateDb::new();
    let nonce = match state.accounts.get(address) {
        Some(account) => account.nonce,
        None => U256::zero(),
    };
    nonce
}

// TODO: currently returns 0 for nonexistent accounts
//       specified behavior is different for more recent patches
pub fn get_balance(address: &Address) -> U256 {
    let state = StateDb::new();
    let balance = match state.accounts.get(address) {
        Some(account) => account.balance,
        None => U256::zero(),
    };
    balance
}

// returns a hex-encoded string directly from storage to avoid unnecessary conversions
pub fn get_code_string(address: &Address) -> String {
    let state = StateDb::new();
    let code = match state.accounts.get(address) {
        Some(account) => account.code.to_string(),
        None => String::new(),
    };
    code
}

fn create_account_state(
    nonce: U256,
    address: Address,
    balance: U256,
    storage: &Storage,
    code: &Rc<Vec<u8>>,
) -> AccountState {
    let mut storage_map: HashMap<U256, U256> = HashMap::new();
    let vm_storage_as_map: alloc::BTreeMap<U256, M256> = storage.clone().into();
    for (key, val) in vm_storage_as_map.iter() {
        let val_as_u256: U256 = val.clone().into();
        storage_map.insert(key.clone(), val_as_u256);
    }

    AccountState {
        nonce: nonce,
        address: address,
        balance: balance,
        storage: storage_map,
        code: to_hex(code),
    }
}

fn update_account_balance<P: Patch>(
    address: &Address,
    amount: U256,
    sign: Sign,
    state: &StateDb,
) -> Option<AccountState> {
    match state.accounts.get(&address) {
        Some(mut account) => {
            // Found account. Update balance.
            account.balance = match sign {
                Sign::Plus => account.balance + amount,
                Sign::Minus => account.balance - amount,
                _ => panic!(),
            };
            Some(account)
        }
        None => {
            // Account doesn't exist; create it.
            assert_eq!(
                sign,
                Sign::Plus,
                "Can't decrease balance of nonexistent account"
            );

            // EIP-161d forbids creating accounts with empty (nonce, code, balance)
            if !P::Account::empty_considered_exists() && amount == U256::from(0) {
                None
            } else {
                Some(AccountState {
                    nonce: P::Account::initial_nonce(),
                    address: address.clone(),
                    balance: amount,
                    storage: HashMap::new(),
                    code: String::new(),
                })
            }
        }
    }
}

pub fn save_transaction_record<P: Patch>(
    hash: H256,
    block_hash: H256,
    block_number: U256,
    index: u32,
    transaction: ValidTransaction,
    vm: &SeqTransactionVM<P>,
) {
    let mut record = TransactionRecord {
        hash: hash,
        nonce: transaction.nonce,
        block_hash: block_hash,
        block_number: block_number,
        index: index,
        from: transaction.caller,
        to: match transaction.action {
            TransactionAction::Call(address) => Some(address),
            TransactionAction::Create => None,
        },
        gas_used: vm.used_gas(),
        cumulative_gas_used: vm.used_gas(),
        value: transaction.value,
        gas_price: transaction.gas_price,
        // TODO: assuming this is gas limit rather than gas used, need to confirm
        gas_provided: transaction.gas_limit,
        input: to_hex(&transaction.input.clone()),
        is_create: false,
        contract_address: None,
        status: false,
        logs: vm.logs().to_vec(),
    };

    for account in vm.accounts() {
        match account {
            &AccountChange::Create {
                nonce,
                address,
                balance,
                ref storage,
                ref code,
            } => {
                if code.len() > 0 {
                    record.is_create = true;
                    record.contract_address = Some(address);
                }
            }
            _ => {}
        }
    }

    match vm.status() {
        VMStatus::ExitedOk => record.status = true,
        _ => record.status = false,
    }

    let state = StateDb::new();
    state.transactions.insert(&hash, &record);
}

pub fn update_state_from_vm<P: Patch>(vm: &SeqTransactionVM<P>) {
    let state = StateDb::new();

    for account in vm.accounts() {
        match account {
            &AccountChange::Create {
                nonce,
                address,
                balance,
                ref storage,
                ref code,
            } => {
                let account_state = create_account_state(nonce, address, balance, storage, code);
                state.accounts.insert(&address, &account_state);
            }
            &AccountChange::Full {
                nonce,
                address,
                balance,
                ref changing_storage,
                ref code,
            } => {
                let mut account_state =
                    create_account_state(nonce, address, balance, changing_storage, code);
                let prev_storage = state.accounts.get(&address).unwrap().storage;

                // This type of change registers a *diff* of the storage, so place previous values
                // in the new map.
                for (key, value) in prev_storage.iter() {
                    if !account_state.storage.contains_key(key) {
                        account_state.storage.insert(key.clone(), value.clone());
                    }
                }

                state.accounts.insert(&address, &account_state);
            }
            &AccountChange::IncreaseBalance(address, amount) => {
                if let Some(new_account) =
                    update_account_balance::<P>(&address, amount, Sign::Plus, &state)
                {
                    state.accounts.insert(&address, &new_account);
                }
            }
            &AccountChange::Nonexist(address) => {}
        }
    }
}

pub fn fire_transaction<P: Patch>(
    transaction: &ValidTransaction,
    block_number: U256,
) -> SeqTransactionVM<P> {
    let state = StateDb::new();

    let block_header = HeaderParams {
        beneficiary: Address::default(),
        timestamp: 0,
        number: block_number,
        difficulty: U256::zero(),
        gas_limit: Gas::zero(),
    };

    let mut vm = SeqTransactionVM::new(transaction.clone(), block_header.clone());

    handle_fire(&mut vm, &state);

    trace!("    VM returned: {:?}", vm.status());
    trace!("    VM out: {:?}", vm.out());

    for account in vm.accounts() {
        trace!("        {:?}", account);
    }

    vm
}

/*
pub fn fire_transactions_and_update_state(
    transactions: &[ValidTransaction],
    block_number: u64,
) -> Vec<u8> {
    let state = StateDb::new();

    let block_header = HeaderParams {
        beneficiary: Address::default(),
        timestamp: 0,
        number: U256::from(block_number),
        difficulty: U256::zero(),
        gas_limit: Gas::zero(),
    };

    let mut last_vm: Option<SeqTransactionVM<MainnetEIP160Patch>> = None;
    for t in transactions.iter() {
        let mut vm = if last_vm.is_none() {
            SeqTransactionVM::new(t.clone(), block_header.clone())
        } else {
            SeqTransactionVM::with_previous(
                t.clone(),
                block_header.clone(),
                last_vm.as_ref().unwrap(),
            )
        };

        handle_fire(&mut vm, &state);

        println!("    VM returned: {:?}", vm.status());
        println!("    VM out: {:?}", vm.out());

        for account in vm.accounts() {
            println!("        {:?}", account);
        }

        last_vm = Some(vm);
    }

    let vm_result = last_vm.as_ref().unwrap().out();

    // TODO: do not update if this is eth_call
    update_state_from_vm(&last_vm.as_ref().unwrap());
    vm_result.to_vec()
}
*/
