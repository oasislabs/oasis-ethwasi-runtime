use rlp;
use block::{Receipt, Block, UnsignedTransaction, Transaction, TransactionAction, Log, FromKey, Header, Account, ommers_hash, transactions_root, receipts_root, RlpHash};
use trie::{MemoryDatabase, Database, MemoryDatabaseGuard, Trie};
use bigint::{H256, M256, U256, H64, B256, Gas, Address};
use bloom::LogsBloom;
use secp256k1::SECP256K1;
use secp256k1::key::{PublicKey, SecretKey};
use std::time::Duration;
use std::thread;
use std::str::FromStr;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::{Arc, Mutex};
use std::rc::Rc;
use evm_api::{ExecuteTransactionRequest, Transaction as EVMTransaction};
use sputnikvm::{AccountChange, ValidTransaction, Patch, AccountCommitment, AccountState, HeaderParams, SeqTransactionVM, VM, VMStatus};
use sputnikvm::errors::RequireError;
use sputnikvm_stateful::MemoryStateful;
use sha3::{Digest, Keccak256};
use blockchain::chain::HeaderHash;
use hexutil::*;

use evm;

mod state;

pub use self::state::MinerState;

pub fn dump_vm<P: Patch>(vm: &SeqTransactionVM<P>) {
    println!("    VM returned: {:?}", vm.status());
    println!("    VM out: {:?}", vm.out());

    for account in vm.accounts() {
        println!("        {:?}", account);
    }
}

fn next<'a>(
    state: &mut MinerState,
    current_block: &Block, transactions: &[Transaction], receipts: &[Receipt],
    beneficiary: Address, gas_limit: Gas, state_root: H256,
) -> Block {
    // TODO: Handle block rewards.

    debug_assert!(transactions.len() == receipts.len());

    let mut logs_bloom = LogsBloom::new();
    let mut gas_used = Gas::zero();

    for i in 0..transactions.len() {
        state.insert_receipt(transactions[i].rlp_hash(), receipts[i].clone());

        logs_bloom = logs_bloom | receipts[i].logs_bloom.clone();
        gas_used = gas_used + receipts[i].used_gas.clone();
    }

    let header = Header {
        parent_hash: current_block.header.header_hash(),
        ommers_hash: ommers_hash(&[]),
        beneficiary,
        state_root: state_root,
        transactions_root: transactions_root(transactions),
        receipts_root: receipts_root(receipts),
        logs_bloom,
        gas_limit,
        gas_used,
        timestamp: current_timestamp(),
        extra_data: B256::default(),
        number: current_block.header.number + U256::one(),

        difficulty: U256::zero(),
        mix_hash: H256::default(),
        nonce: H64::default(),
    };

    Block {
        header,
        transactions: transactions.into(),
        ommers: Vec::new()
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

lazy_static! {
    static ref DATABASE: MemoryDatabase = MemoryDatabase::default();
}

pub fn make_state<P: Patch>(genesis_accounts: Vec<(SecretKey, U256)>) -> MinerState {
    let mut stateful = MemoryStateful::empty(&DATABASE);
    let mut genesis = Block {
        header: Header {
            parent_hash: H256::default(),
            // TODO: use the known good result from etclient
            ommers_hash: MemoryDatabase::default().create_empty().root(),
            beneficiary: Address::default(),
            state_root: stateful.root(),
            transactions_root: MemoryDatabase::default().create_empty().root(),
            receipts_root: MemoryDatabase::default().create_empty().root(),
            logs_bloom: LogsBloom::new(),
            number: U256::zero(),
            gas_limit: Gas::zero(),
            gas_used: Gas::zero(),
            timestamp: current_timestamp(),
            extra_data: B256::default(),
            difficulty: U256::zero(),
            mix_hash: H256::default(),
            nonce: H64::default(),
        },
        transactions: Vec::new(),
        ommers: Vec::new(),
    };

    let mut all_account_changes = Vec::new();
    for &(ref secret_key, balance) in &genesis_accounts {
        let address = Address::from_secret_key(secret_key).unwrap();

        let vm: SeqTransactionVM<P> = {
            let vm = stateful.call(ValidTransaction {
                caller: None,
                gas_price: Gas::zero(),
                gas_limit: Gas::from(100000usize),
                action: TransactionAction::Call(address),
                value: balance,
                input: Rc::new(Vec::new()),
                nonce: U256::zero(),
            }, HeaderParams::from(&genesis.header), &[]);
            let mut accounts = Vec::new();
            for account in vm.accounts() {
                accounts.push(account.clone());
            }
            stateful.transit(&accounts);
            all_account_changes.push(accounts);
            vm
        };
    }

    genesis.header.state_root = stateful.root();

    let mut state = MinerState::new(genesis, stateful);

    for (secret_key, balance) in genesis_accounts {
        let address = Address::from_secret_key(&secret_key).unwrap();
        //println!("address: {:?}", address);
        //println!("private key: {}", to_hex(&secret_key[..]));

        state.append_account(secret_key);
        for accounts in &all_account_changes {
            state.fat_transit(0, &accounts);
        }
    }

    state
}
