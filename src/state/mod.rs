extern crate alloc;
extern crate bigint;
extern crate hexutil;
extern crate sha3;
extern crate sputnikvm;

use ekiden_trusted::db::database_schema;

use std::collections::HashMap;

use bigint::{Address, H256, M256, Sign, U256};

use evm_api::{AccountState, Block, TransactionRecord};
use hexutil::to_hex;

use sputnikvm::{AccountChange, AccountPatch, Patch, SeqTransactionVM, Storage, TransactionAction,
                VMStatus, ValidTransaction, VM};

use std::rc::Rc;

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

pub fn create_account_state(
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

pub fn update_account_balance<P: Patch>(
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
