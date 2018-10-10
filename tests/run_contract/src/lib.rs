extern crate either;
extern crate ekiden_core;
extern crate ekiden_roothash_base;
extern crate ekiden_trusted;
extern crate ethcore;
extern crate ethereum_api;
extern crate ethereum_types;
extern crate ethkey;
#[macro_use]
extern crate lazy_static;
extern crate runtime_ethereum;

use std::str::FromStr;

use either::Either;
use ekiden_roothash_base::Header;
use ekiden_trusted::{contract::dispatcher::{BatchHandler, ContractCallContext},
                     db::{Database, DatabaseHandle}};
use ethcore::{rlp,
              storage::Storage,
              transaction::{Action, SignedTransaction, Transaction}};
use ethereum_api::{ExecuteTransactionResponse, Receipt};
use ethereum_types::{Address, H256, U256};
use ethkey::Secret;
use runtime_ethereum::{execute_raw_transaction, get_account_nonce, get_receipt,
                       storage::GlobalStorage, EthereumBatchHandler, EthereumContext};

fn dummy_ctx() -> ContractCallContext {
    let root_hash = DatabaseHandle::instance().get_root_hash();
    ContractCallContext {
        header: Header {
            timestamp: 0xcafedeadbeefc0de,
            ..Default::default()
        },
        runtime: EthereumContext::new(root_hash),
    }
}

fn with_batch_handler<F, R>(f: F) -> R
where
    F: FnOnce(&mut ContractCallContext) -> R,
{
    let mut ctx = dummy_ctx();
    let batch_handler = EthereumBatchHandler;
    batch_handler.start_batch(&mut ctx);

    let result = f(&mut ctx);

    batch_handler.end_batch(ctx);

    result
}

lazy_static! {
    static ref DEFAULT_ACCOUNT: Address = Address::from("1cca28600d7491365520b31b466f88647b9839ec");
    static ref SECRET_KEY: Secret = Secret::from_str(
        // private key corresponding to DEFAULT_ACCOUNT. generated from mnemonic:
        // patient oppose cotton portion chair gentle jelly dice supply salmon blast priority
        "c61675c22aee77da8f6e19444ece45557dc80e1482aa848f541e94e3e5d91179"
    ).unwrap();
}

/// Makes a transaction.
/// Either a CREATE containing the contract bytes or a CALL to an address with some data bytes.
pub fn make_tx(spec: Either<Vec<u8>, (Address, Vec<u8>)>) -> SignedTransaction {
    let mut tx = Transaction::default();
    tx.gas = U256::from("1e84800");
    tx.nonce = U256::from(get_account_nonce(&DEFAULT_ACCOUNT, &mut dummy_ctx()).unwrap());
    match spec {
        Either::Left(data) => tx.data = data,
        Either::Right((addr, data)) => {
            tx.action = Action::Call(addr);
            tx.data = data;
        }
    };
    tx.sign(&SECRET_KEY, None)
}

/// Runs a signed transaction using the runtime.
pub fn run_tx(tx: SignedTransaction) -> Result<Receipt, ExecuteTransactionResponse> {
    let res = with_batch_handler(|ctx| {
        execute_raw_transaction(&(rlp::encode(&tx).to_vec(), false), ctx).unwrap()
    });
    let receipt = with_batch_handler(|ctx| {
        get_receipt(res.hash.as_ref().unwrap(), ctx)
            .unwrap()
            .unwrap()
    });
    if !receipt.status_code.is_some() || receipt.status_code.unwrap() == 0 {
        println!("ERROR:\n{:?}\n{:?}", res, receipt);
        Err(res)
    } else {
        Ok(receipt)
    }
}

pub fn store_bytes(bytes: &[u8]) -> H256 {
    GlobalStorage::new()
        .store_bytes(bytes)
        .expect("Could not store bytes.")
}

pub fn fetch_bytes(key: &H256) -> Vec<u8> {
    GlobalStorage::new()
        .fetch_bytes(key)
        .expect("Could not fetch bytes.")
}
