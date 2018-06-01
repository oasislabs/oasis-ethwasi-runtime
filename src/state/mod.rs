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
        pub account_storage: Map<(Address, U256), M256>,
        pub transactions: Map<H256, TransactionRecord>,
        pub latest_block_number: U256,
        pub blocks: Map<U256, Block>,
    }
}

pub fn get_account_state(address: Address) -> Option<AccountState> {
    StateDb::new().accounts.get(&address)
}

pub fn get_account_storage(address: Address, index: U256) -> M256 {
    let state = StateDb::new();
    let value = match state.account_storage.get(&(address, index)) {
        Some(val) => val.clone(),
        None => M256::zero(),
    };
    value
}

// TODO: currently returns 0 for nonexistent accounts
//       specified behavior is different for more recent patches
pub fn get_account_nonce(address: &Address) -> U256 {
    let state = StateDb::new();
    let nonce = match state.accounts.get(address) {
        Some(account) => account.nonce,
        None => U256::zero(),
    };
    nonce
}

// TODO: currently returns 0 for nonexistent accounts
//       specified behavior is different for more recent patches
pub fn get_account_balance(address: &Address) -> U256 {
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

pub fn update_account_state(nonce: U256, address: Address, balance: U256, code: &Rc<Vec<u8>>) {
    let account_state = AccountState {
        nonce: nonce,
        address: address,
        balance: balance,
        code: to_hex(code),
    };

    StateDb::new().accounts.insert(&address, &account_state);
}

pub fn update_account_storage(address: Address, storage: &Storage) {
    let state = StateDb::new();
    let storage: HashMap<U256, M256> = storage.clone().into();
    for (key, val) in storage {
        state.account_storage.insert(&(address, key), &val);
    }
}

pub fn update_account_balance<P: Patch>(address: &Address, amount: U256, sign: Sign) {
    let state = StateDb::new();
    match state.accounts.get(&address) {
        Some(mut account) => {
            // Found account. Update balance.
            account.balance = match sign {
                Sign::Plus => account.balance + amount,
                Sign::Minus => account.balance - amount,
                _ => panic!(),
            };
            state.accounts.insert(&address, &account);
        }
        None => {
            // Account doesn't exist; create it.
            assert_eq!(
                sign,
                Sign::Plus,
                "Can't decrease balance of nonexistent account"
            );

            // EIP-161d forbids creating accounts with empty (nonce, code, balance)
            if P::Account::empty_considered_exists() || amount != U256::from(0) {
                let account_state = AccountState {
                    nonce: P::Account::initial_nonce(),
                    address: address.clone(),
                    balance: amount,
                    code: String::new(),
                };
                state.accounts.insert(&address, &account_state);
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
