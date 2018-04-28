extern crate alloc;
extern crate bigint;
extern crate hexutil;
extern crate sha3;
extern crate sputnikvm;

use ekiden_trusted::db::database_schema;

use std::collections::HashMap;

use bigint::{Address, Gas, H256, M256, Sign, U256};

use evm_api::AccountState;
use hexutil::{read_hex, to_hex};

use sputnikvm::{AccountChange, AccountCommitment, HeaderParams, MainnetEIP160Patch, RequireError,
                SeqTransactionVM, Storage, ValidTransaction, VM};
use std::str::FromStr;

use std::rc::Rc;

// Create database schema.
database_schema! {
    pub struct StateDb {
        pub accounts: Map<String, AccountState>,
    }
}

fn handle_fire(vm: &mut SeqTransactionVM<MainnetEIP160Patch>, state: &StateDb) {
    loop {
        match vm.fire() {
            Ok(()) => break,
            Err(RequireError::Account(address)) => {
                let addr_str = address.hex();
                let commit = match state.accounts.get(&addr_str) {
                    Some(b) => {
                        let result = AccountCommitment::Full {
                            nonce: U256::from_dec_str(b.get_nonce()).unwrap(),
                            address: address,
                            balance: U256::from_dec_str(b.get_balance()).unwrap(),
                            code: Rc::new(read_hex(b.get_code()).unwrap()),
                        };
                        result
                    }
                    None => AccountCommitment::Nonexist(address),
                };
                vm.commit_account(commit).unwrap();
            }
            Err(RequireError::AccountStorage(address, index)) => {
                let addr_str = address.hex();
                let index_str = format!("{}", index);

                let value = match state
                    .accounts
                    .get(&addr_str)
                    .unwrap()
                    .storage
                    .get(&index_str)
                {
                    Some(b) => M256(U256::from_dec_str(b).unwrap()),
                    None => M256::zero(),
                };

                vm.commit_account(AccountCommitment::Storage {
                    address: address,
                    index: index,
                    value: value,
                }).unwrap();
            }
            Err(RequireError::AccountCode(address)) => {
                vm.commit_account(AccountCommitment::Nonexist(address))
                    .unwrap();
            }
            Err(RequireError::Blockhash(number)) => {
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

pub fn get_nonce(address: String) -> U256 {
    let state = StateDb::new();
    let nonce = match state.accounts.get(&address) {
        Some(b) => U256::from_dec_str(b.get_nonce()).unwrap(),
        None => U256::zero(),
    };
    nonce
}

fn create_account_state(
    nonce: U256,
    address: Address,
    balance: U256,
    storage: &Storage,
    code: &Rc<Vec<u8>>,
) -> (String, AccountState) {
    let mut storage_map: HashMap<String, String> = HashMap::new();
    let vm_storage_as_map: alloc::BTreeMap<U256, M256> = storage.clone().into();
    for (key, val) in vm_storage_as_map.iter() {
        let val_as_u256: U256 = val.clone().into();
        storage_map.insert(format!("{}", key), format!("{}", val_as_u256));
    }

    let address_str = address.hex();
    let mut account_state = AccountState::new();

    account_state.set_nonce(format!("{}", nonce));
    account_state.set_address(address_str.clone());
    account_state.set_balance(format!("{}", balance));
    account_state.set_storage(storage_map);
    account_state.set_code(to_hex(code));

    (address_str, account_state)
}

fn update_account_balance(
    address_str: &String,
    amount: U256,
    sign: Sign,
    state: &StateDb,
) -> AccountState {
    match state.accounts.get(address_str) {
        Some(b) => {
            // Found account. Update balance.
            let mut updated_account = b.clone();
            let prev_balance: U256 = U256::from_str(b.get_balance()).unwrap();
            let new_balance = match sign {
                Sign::Plus => prev_balance + amount,
                Sign::Minus => prev_balance - amount,
                _ => panic!(),
            };
            updated_account.set_balance(format!("{}", new_balance));
            updated_account
        }
        None => {
            // Account doesn't exist; create it.
            assert_eq!(
                sign,
                Sign::Plus,
                "Can't decrease balance of nonexistent account"
            );
            let mut account_state = AccountState::new();
            account_state.set_nonce("0".to_string());
            account_state.set_address(address_str.clone());
            account_state.set_balance(format!("{}", amount));
            account_state
        }
    }
}

pub fn update_state_from_vm(vm: &SeqTransactionVM<MainnetEIP160Patch>) {
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
                let (addr_str, account_state) =
                    create_account_state(nonce, address, balance, storage, code);
                state.accounts.insert(&addr_str, &account_state);
            }
            &AccountChange::Full {
                nonce,
                address,
                balance,
                ref changing_storage,
                ref code,
            } => {
                let (addr_str, mut account_state) =
                    create_account_state(nonce, address, balance, changing_storage, code);
                let prev_storage = state.accounts.get(&addr_str).unwrap().storage;

                // This type of change registers a *diff* of the storage, so place previous values
                // in the new map.
                for (key, value) in prev_storage.iter() {
                    if !account_state.storage.contains_key(key) {
                        account_state
                            .mut_storage()
                            .insert(key.clone(), value.clone());
                    }
                }

                state.accounts.insert(&addr_str, &account_state);
            }
            &AccountChange::IncreaseBalance(address, amount) => {
                let address_str = address.hex();
                let new_account = update_account_balance(&address_str, amount, Sign::Plus, &state);
                state.accounts.insert(&address_str, &new_account);
            }
            &AccountChange::DecreaseBalance(address, amount) => {
                let address_str = address.hex();
                let new_account = update_account_balance(&address_str, amount, Sign::Minus, &state);
                state.accounts.insert(&address_str, &new_account);
            }
            &AccountChange::Nonexist(address) => {
                panic!("Unexpected nonexistent address: {:?}", address)
            }
        }
    }
}

pub fn fire_transaction(
    transaction: &ValidTransaction,
    block_number: u64,
) -> SeqTransactionVM<MainnetEIP160Patch> {
    let state = StateDb::new();

    let block_header = HeaderParams {
        beneficiary: Address::default(),
        timestamp: 0,
        number: U256::from(block_number),
        difficulty: U256::zero(),
        gas_limit: Gas::zero(),
    };

    let mut vm = SeqTransactionVM::new(transaction.clone(), block_header.clone());

    handle_fire(&mut vm, &state);

    println!("    VM returned: {:?}", vm.status());
    println!("    VM out: {:?}", vm.out());

    for account in vm.accounts() {
        println!("        {:?}", account);
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
