#![feature(int_to_from_bytes)]

extern crate common_types as ethcore_types;
extern crate ekiden_common;
extern crate ekiden_core;
extern crate ekiden_storage_base;
extern crate ekiden_storage_dummy;
extern crate ekiden_storage_lru;
extern crate ekiden_trusted;
extern crate elastic_array;
extern crate ethcore;
extern crate ethereum_api;
extern crate ethereum_types;
extern crate hashdb;
extern crate hex;
extern crate keccak_hash;
#[macro_use]
extern crate lazy_static;
extern crate log;
extern crate protobuf;
extern crate runtime_ethereum_common;
extern crate sha3;

mod evm;
#[macro_use]
mod logger;
mod state;
#[cfg(debug_assertions)]
pub mod storage; // allow access from tests/run_contract
#[cfg(not(debug_assertions))]
mod storage;

use std::sync::Arc;

use ekiden_core::error::{Error, Result};
use ekiden_storage_base::StorageBackend;
#[cfg(not(target_env = "sgx"))]
use ekiden_storage_dummy::DummyStorageBackend;
#[cfg(target_env = "sgx")]
use ekiden_trusted::db::untrusted::UntrustedStorageBackend;
use ekiden_trusted::{db::{Database, DatabaseHandle},
                     enclave::enclave_init,
                     runtime::{configure_runtime_dispatch_batch_handler,
                               create_runtime,
                               dispatcher::{BatchHandler, RuntimeCallContext}}};
use ethcore::{block::{IsBlock, OpenBlock},
              error::BlockError,
              log_entry::LogEntry as EthLogEntry,
              receipt::Receipt as EthReceipt,
              rlp,
              transaction::{Action, SignedTransaction, Transaction as EthcoreTransaction,
                            UnverifiedTransaction}};
use ethereum_api::{with_api, AccountState, BlockId, ExecuteTransactionResponse, Filter, Log,
                   Receipt, SimulateTransactionResponse, Transaction, TransactionRequest};
use ethereum_types::{Address, H256, U256};

use self::state::Cache;
use self::storage::GlobalStorage;

enclave_init!();

// Create enclave runtime interface.
with_api! {
    create_runtime!(api);
}

/// Ethereum-specific batch context.
pub struct EthereumContext<'a> {
    /// Blockchain cache.
    cache: Cache,
    /// Currently open block.
    block: OpenBlock<'a>,
    /// Force emitting a block.
    force_emit_block: bool,
}

impl<'a> EthereumContext<'a> {
    /// Create new Ethereum-specific batch context.
    pub fn new(storage: Arc<StorageBackend>, db: DatabaseHandle) -> Box<Self> {
        let cache = Cache::from_global(storage, db);

        Box::new(EthereumContext {
            block: cache.new_block().unwrap(),
            cache,
            force_emit_block: false,
        })
    }
}

#[cfg(target_env = "sgx")]
pub struct EthereumBatchHandler;
#[cfg(not(target_env = "sgx"))]
pub struct EthereumBatchHandler {
    /// Allow to configure the storage backend in non-SGX environments.
    pub storage: Arc<StorageBackend>,
}

impl BatchHandler for EthereumBatchHandler {
    fn start_batch(&self, ctx: &mut RuntimeCallContext) {
        // Obtain current root hash from the block header.
        let root_hash = ctx.header.state_root;

        // Create a new storage backend.
        #[cfg(target_env = "sgx")]
        let storage = Arc::new(UntrustedStorageBackend::new());
        #[cfg(not(target_env = "sgx"))]
        let storage = self.storage.clone();

        // Create a fresh database instance for the given root hash.
        let mut db = DatabaseHandle::new(storage.clone());
        db.set_root_hash(root_hash).unwrap();

        ctx.runtime = EthereumContext::new(storage, db);
    }

    fn end_batch(&self, ctx: RuntimeCallContext) {
        let timestamp = ctx.header.timestamp;
        let mut ectx = *ctx.runtime.downcast::<EthereumContext>().unwrap();

        // Finalize the block if it contains any transactions.
        if !ectx.block.transactions().is_empty() || ectx.force_emit_block {
            ectx.block.set_timestamp(timestamp);
            ectx.cache.add_block(ectx.block.close_and_lock()).unwrap();
        }

        // Update cached value.
        let root_hash = ectx.cache.commit_global();

        // TODO: Get rid of the global database handle instance.
        DatabaseHandle::instance().set_root_hash(root_hash).unwrap();
    }
}

configure_runtime_dispatch_batch_handler!(EthereumBatchHandler);

// used for performance debugging
fn debug_null_call(_request: &bool, _ctx: &RuntimeCallContext) -> Result<()> {
    Ok(())
}

fn strip_0x<'a>(hex: &'a str) -> &'a str {
    if hex.starts_with("0x") {
        hex.get(2..).unwrap()
    } else {
        hex
    }
}

fn from_hex<S: AsRef<str>>(hex: S) -> Result<Vec<u8>> {
    Ok(hex::decode(strip_0x(hex.as_ref()))?)
}

#[cfg(any(debug_assertions, feature = "benchmark"))]
fn inject_accounts(accounts: &Vec<AccountState>, ctx: &mut RuntimeCallContext) -> Result<()> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    accounts.iter().try_for_each(|ref account| {
        ectx.block.block_mut().state_mut().new_contract(
            &account.address,
            account.balance.clone(),
            account.nonce.clone(),
        );
        if account.code.len() > 0 {
            ectx.block
                .block_mut()
                .state_mut()
                .init_code(&account.address, from_hex(&account.code)?)
                .map_err(|_| {
                    Error::new(format!(
                        "Could not init code for address {:?}.",
                        &account.address
                    ))
                })
        } else {
            Ok(())
        }
    })?;

    // Force finalization as this block doesn't include transactions.
    ectx.force_emit_block = true;

    Ok(())
}

#[cfg(not(any(debug_assertions, feature = "benchmark")))]
fn inject_accounts(accounts: &Vec<AccountState>, _ctx: &RuntimeCallContext) -> Result<()> {
    Err(Error::new(
        "API available only in debug and benchmarking builds",
    ))
}

#[cfg(any(debug_assertions, feature = "benchmark"))]
pub fn inject_account_storage(
    storages: &Vec<(Address, H256, H256)>,
    ctx: &mut RuntimeCallContext,
) -> Result<()> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    storages.iter().try_for_each(|&(addr, key, value)| {
        ectx.block
            .block_mut()
            .state_mut()
            .set_storage(&addr, key.clone(), value.clone())
            .map_err(|_| Error::new("Could not set storage."))
    })?;

    // Force finalization as this block doesn't include transactions.
    ectx.force_emit_block = true;

    Ok(())
}

#[cfg(not(any(debug_assertions, feature = "benchmark")))]
fn inject_account_storage(
    storage: &Vec<(Address, H256, H256)>,
    _ctx: &RuntimeCallContext,
) -> Result<()> {
    Err(Error::new(
        "API available only in debug and benchmarking builds",
    ))
}

/// TODO: first argument is ignored; remove once APIs support zero-argument signatures (#246)
pub fn get_block_height(_request: &bool, ctx: &mut RuntimeCallContext) -> Result<U256> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    Ok(ectx.cache.get_latest_block_number().into())
}

fn get_block_hash(id: &BlockId, ctx: &mut RuntimeCallContext) -> Result<Option<H256>> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    let hash = match *id {
        BlockId::Hash(hash) => Some(hash),
        BlockId::Number(number) => ectx.cache.block_hash(number.into()),
        BlockId::Earliest => ectx.cache.block_hash(0),
        BlockId::Latest => ectx.cache.block_hash(ectx.cache.get_latest_block_number()),
    };
    Ok(hash)
}

fn get_block(id: &BlockId, ctx: &mut RuntimeCallContext) -> Result<Option<Vec<u8>>> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    debug!("get_block, id: {:?}", id);

    let block = match *id {
        BlockId::Hash(hash) => ectx.cache.block_by_hash(hash),
        BlockId::Number(number) => ectx.cache.block_by_number(number.into()),
        BlockId::Earliest => ectx.cache.block_by_number(0),
        BlockId::Latest => ectx.cache
            .block_by_number(ectx.cache.get_latest_block_number()),
    };

    match block {
        Some(block) => Ok(Some(block.into_inner())),
        None => Ok(None),
    }
}

fn get_logs(filter: &Filter, ctx: &mut RuntimeCallContext) -> Result<Vec<Log>> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    debug!("get_logs, filter: {:?}", filter);
    Ok(ectx.cache.get_logs(filter))
}

pub fn get_transaction(hash: &H256, ctx: &mut RuntimeCallContext) -> Result<Option<Transaction>> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    debug!("get_transaction, hash: {:?}", hash);
    Ok(ectx.cache.get_transaction(hash))
}

pub fn get_receipt(hash: &H256, ctx: &mut RuntimeCallContext) -> Result<Option<Receipt>> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    debug!("get_receipt, hash: {:?}", hash);
    Ok(ectx.cache.get_receipt(hash))
}

pub fn get_account_balance(address: &Address, ctx: &mut RuntimeCallContext) -> Result<U256> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    debug!("get_account_balance, address: {:?}", address);
    ectx.cache.get_account_balance(address)
}

pub fn get_account_nonce(address: &Address, ctx: &mut RuntimeCallContext) -> Result<U256> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    debug!("get_account_nonce, address: {:?}", address);
    ectx.cache.get_account_nonce(address)
}

pub fn get_account_code(
    address: &Address,
    ctx: &mut RuntimeCallContext,
) -> Result<Option<Vec<u8>>> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    debug!("get_account_code, address: {:?}", address);
    ectx.cache.get_account_code(address)
}

pub fn get_storage_at(pair: &(Address, H256), ctx: &mut RuntimeCallContext) -> Result<H256> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    debug!("get_storage_at, address: {:?}", pair);
    ectx.cache.get_account_storage(pair.0, pair.1)
}

pub fn execute_raw_transaction(
    request: &Vec<u8>,
    ctx: &mut RuntimeCallContext,
) -> Result<ExecuteTransactionResponse> {
    let mut ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    debug!("execute_raw_transaction");

    let decoded: UnverifiedTransaction = match rlp::decode(request) {
        Ok(t) => t,
        Err(e) => {
            return Ok(ExecuteTransactionResponse {
                hash: Err(e.to_string()),
                created_contract: false,
            })
        }
    };

    let is_create = decoded.as_unsigned().action == Action::Create;
    let signed = match SignedTransaction::new(decoded) {
        Ok(t) => t,
        Err(e) => {
            return Ok(ExecuteTransactionResponse {
                hash: Err(e.to_string()),
                created_contract: false,
            })
        }
    };
    let result = transact(&mut ectx, signed).map_err(|e| e.to_string());
    Ok(ExecuteTransactionResponse {
        created_contract: if result.is_err() { false } else { is_create },
        hash: result,
    })
}

fn transact(ectx: &mut EthereumContext, transaction: SignedTransaction) -> Result<H256> {
    let tx_hash = transaction.hash();
    let mut storage = GlobalStorage::new();
    ectx.block
        .push_transaction(transaction, None, &mut storage)?;
    Ok(tx_hash)
}

fn make_unsigned_transaction(
    cache: &Cache,
    request: &TransactionRequest,
) -> Result<SignedTransaction> {
    let tx = EthcoreTransaction {
        action: if request.is_call {
            Action::Call(request
                .address
                .ok_or(Error::new("Must provide address for call transaction."))?)
        } else {
            Action::Create
        },
        value: request.value.unwrap_or(U256::zero()),
        data: request.input.clone().unwrap_or(vec![]),
        gas: U256::max_value(),
        gas_price: U256::zero(),
        nonce: request.nonce.unwrap_or_else(|| {
            request
                .caller
                .map(|addr| cache.get_account_nonce(&addr).unwrap_or(U256::zero()))
                .unwrap_or(U256::zero())
        }),
    };
    Ok(match request.caller {
        Some(addr) => tx.fake_sign(addr),
        None => tx.null_sign(0),
    })
}

pub fn simulate_transaction(
    request: &TransactionRequest,
    ctx: &mut RuntimeCallContext,
) -> Result<SimulateTransactionResponse> {
    let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

    debug!("simulate_transaction");
    let tx = match make_unsigned_transaction(&ectx.cache, request) {
        Ok(t) => t,
        Err(e) => {
            return Ok(SimulateTransactionResponse {
                used_gas: U256::from(0),
                refunded_gas: U256::from(0),
                result: Err(e.to_string()),
            })
        }
    };
    let exec = match evm::simulate_transaction(&ectx.cache, &tx) {
        Ok(exec) => exec,
        Err(e) => {
            return Ok(SimulateTransactionResponse {
                used_gas: U256::from(0),
                refunded_gas: U256::from(0),
                result: Err(e.to_string()),
            })
        }
    };

    Ok(SimulateTransactionResponse {
        used_gas: exec.gas_used,
        refunded_gas: exec.refunded,
        result: Ok(exec.output),
    })
}

#[cfg(test)]
mod tests {
    extern crate ekiden_roothash_base;
    extern crate ekiden_storage_base;
    extern crate ekiden_storage_dummy;
    extern crate ethkey;

    use std::str::FromStr;
    use std::sync::Arc;
    use std::sync::Mutex;

    use self::ekiden_common::futures::Future;
    use self::ekiden_core::bytes::H256 as EkidenH256;
    use self::ekiden_roothash_base::header::Header;
    use self::ekiden_storage_base::StorageBackend;
    use self::ekiden_storage_dummy::DummyStorageBackend;
    use self::ethkey::{KeyPair, Secret};

    use super::*;
    use ethcore::{self, blockchain::BlockChain, vm};
    use hex;

    lazy_static! {
        // Global dummy storage used in tests.
        static ref STORAGE: Arc<StorageBackend> = Arc::new(DummyStorageBackend::new());

        // Genesis block state root as Ekiden H256.
        static ref GENESIS_STATE_ROOT: EkidenH256 =
            EkidenH256::from_slice(&evm::SPEC.state_root().to_vec());
    }

    fn dummy_ctx() -> RuntimeCallContext {
        let root_hash = DatabaseHandle::instance().get_root_hash();
        let mut ctx = RuntimeCallContext::new(Header {
            timestamp: 0xcafedeadbeefc0de,
            state_root: root_hash,
            ..Default::default()
        });

        // Initialize the context in the same way as a batch handler does.
        let batch_handler = EthereumBatchHandler {
            storage: STORAGE.clone(),
        };
        batch_handler.start_batch(&mut ctx);

        ctx
    }

    fn with_batch_handler<F, R>(f: F) -> R
    where
        F: FnOnce(&mut RuntimeCallContext) -> R,
    {
        let root_hash = DatabaseHandle::instance().get_root_hash();
        let mut ctx = RuntimeCallContext::new(Header {
            timestamp: 0xcafedeadbeefc0de,
            state_root: root_hash,
            ..Default::default()
        });

        let batch_handler = EthereumBatchHandler {
            storage: STORAGE.clone(),
        };
        batch_handler.start_batch(&mut ctx);

        let result = f(&mut ctx);

        batch_handler.end_batch(ctx);

        // Check that the genesis block state root is in storage.
        // It should always exist in storage after any batch has committed.
        assert!(STORAGE.get(*GENESIS_STATE_ROOT).wait().is_ok());

        result
    }

    struct Client {
        keypair: KeyPair,
    }

    impl Client {
        fn new() -> Self {
            Self {
                // address: 0x7110316b618d20d0c44728ac2a3d683536ea682
                keypair: KeyPair::from_secret(
                    Secret::from_str(
                        "533d62aea9bbcb821dfdda14966bb01bfbbb53b7e9f5f0d69b8326e052e3450c",
                    ).unwrap(),
                ).unwrap(),
            }
        }

        fn create_contract(&mut self, code: Vec<u8>, balance: &U256) -> (H256, Address) {
            let hash = with_batch_handler(|ctx| {
                let tx = EthcoreTransaction {
                    action: Action::Create,
                    nonce: get_account_nonce(&self.keypair.address(), ctx).unwrap(),
                    gas_price: U256::from(0),
                    gas: U256::from(1000000),
                    value: *balance,
                    data: code,
                }.sign(&self.keypair.secret(), None);

                let raw = rlp::encode(&tx);
                execute_raw_transaction(&raw.into_vec(), ctx)
                    .unwrap()
                    .hash
                    .unwrap()
            });

            let receipt = with_batch_handler(|ctx| get_receipt(&hash, ctx).unwrap().unwrap());
            (hash, receipt.contract_address.unwrap())
        }

        fn call(&mut self, contract: &Address, data: Vec<u8>, value: &U256) -> Vec<u8> {
            let tx = TransactionRequest {
                caller: Some(self.keypair.address()),
                is_call: true,
                address: Some(*contract),
                input: Some(data),
                value: Some(*value),
                nonce: None,
            };

            with_batch_handler(|ctx| simulate_transaction(&tx, ctx).unwrap().result.unwrap())
        }
    }

    lazy_static! {
        static ref CLIENT: Mutex<Client> = Mutex::new(Client::new());
    }

    #[test]
    fn test_create_balance() {
        let mut client = CLIENT.lock().unwrap();

        let init_bal = get_account_balance(&client.keypair.address(), &mut dummy_ctx()).unwrap();
        let contract_bal = U256::from(10);
        let remaining_bal = init_bal - contract_bal;

        let init_nonce = get_account_nonce(&client.keypair.address(), &mut dummy_ctx()).unwrap();

        let code = hex::decode("3331600055").unwrap(); // SSTORE(0x0, BALANCE(CALLER()))
        let (_, contract) = client.create_contract(code, &contract_bal);

        assert_eq!(
            get_account_balance(&client.keypair.address(), &mut dummy_ctx()).unwrap(),
            remaining_bal
        );
        assert_eq!(
            get_account_nonce(&client.keypair.address(), &mut dummy_ctx()).unwrap(),
            init_nonce + U256::one()
        );
        assert_eq!(
            get_account_balance(&contract, &mut dummy_ctx()).unwrap(),
            contract_bal
        );
        assert_eq!(
            get_storage_at(&(contract, H256::zero()), &mut dummy_ctx()).unwrap(),
            H256::from(&remaining_bal)
        );
    }

    #[test]
    fn test_solidity_blockhash() {
        // pragma solidity ^0.4.18;
        // contract The {
        //   function hash(uint64 num) public view returns (bytes32) {
        //     return blockhash(num);
        //   }
        // }

        use std::mem::transmute;

        let mut client = CLIENT.lock().unwrap();
        let blockhash_code = hex::decode("608060405234801561001057600080fd5b5060d58061001f6000396000f300608060405260043610603f576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063e432a10e146044575b600080fd5b348015604f57600080fd5b506076600480360381019080803567ffffffffffffffff1690602001909291905050506094565b60405180826000191660001916815260200191505060405180910390f35b60008167ffffffffffffffff164090509190505600a165627a7a7230582078c16bf994a1597df9b750bb680f3fc4b4e8c9c8f51607bbfcc28d9496a211d70029").unwrap();

        let (_, contract) = client.create_contract(blockhash_code, &U256::zero());

        let mut blockhash = |num: u64| -> Vec<u8> {
            let mut data = hex::decode(
                "e432a10e0000000000000000000000000000000000000000000000000000000000000000",
            ).unwrap();
            let bytes: [u8; 8] = unsafe { transmute(num.to_be()) };
            for i in 0..8 {
                data[28 + i] = bytes[i];
            }
            client.call(&contract, data, &U256::zero())
        };

        let block_number = with_batch_handler(|ctx| {
            let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();
            ectx.cache.get_latest_block_number()
        });
        let client_blockhash = blockhash(block_number);

        with_batch_handler(|ctx| {
            let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();
            assert_eq!(
                client_blockhash,
                ectx.cache
                    .block_hash(ectx.cache.get_latest_block_number())
                    .unwrap()
                    .to_vec()
            );
        });
    }

    #[test]
    fn test_solidity_x_contract_call() {
        // contract A {
        //   function call_a(address b, int a) public pure returns (int) {
        //       B cb = B(b);
        //       return cb.call_b(a);
        //     }
        // }
        //
        // contract B {
        //     function call_b(int b) public pure returns (int) {
        //             return b + 1;
        //         }
        // }

        let mut client = CLIENT.lock().unwrap();

        let contract_a_code = hex::decode("608060405234801561001057600080fd5b5061015d806100206000396000f3006080604052600436106100405763ffffffff7c0100000000000000000000000000000000000000000000000000000000600035041663e3f300558114610045575b600080fd5b34801561005157600080fd5b5061007673ffffffffffffffffffffffffffffffffffffffff60043516602435610088565b60408051918252519081900360200190f35b6000808390508073ffffffffffffffffffffffffffffffffffffffff1663346fb5c9846040518263ffffffff167c010000000000000000000000000000000000000000000000000000000002815260040180828152602001915050602060405180830381600087803b1580156100fd57600080fd5b505af1158015610111573d6000803e3d6000fd5b505050506040513d602081101561012757600080fd5b50519493505050505600a165627a7a7230582062a004e161bd855be0a78838f92bafcbb4cef5df9f9ac673c2f7d174eff863fb0029").unwrap();
        let (_, contract_a) = client.create_contract(contract_a_code, &U256::zero());

        let contract_b_code = hex::decode("6080604052348015600f57600080fd5b50609c8061001e6000396000f300608060405260043610603e5763ffffffff7c0100000000000000000000000000000000000000000000000000000000600035041663346fb5c981146043575b600080fd5b348015604e57600080fd5b506058600435606a565b60408051918252519081900360200190f35b600101905600a165627a7a72305820ea09447c835e5eb442e1a85e271b0ae6decf8551aa73948ab6b53e8dd1fa0dca0029").unwrap();
        let (_, contract_b) = client.create_contract(contract_b_code, &U256::zero());

        let data = hex::decode(format!(
            "e3f30055000000000000000000000000{:\
             x}0000000000000000000000000000000000000000000000000000000000000029",
            contract_b
        )).unwrap();
        let output = client.call(&contract_a, data, &U256::zero());

        // expected output is 42
        assert_eq!(
            hex::encode(output),
            "000000000000000000000000000000000000000000000000000000000000002a"
        );
    }

    #[test]
    fn test_redeploy() {
        let mut client = CLIENT.lock().unwrap();

        let contract_code = hex::decode("6080604052348015600f57600080fd5b50609c8061001e6000396000f300608060405260043610603e5763ffffffff7c0100000000000000000000000000000000000000000000000000000000600035041663346fb5c981146043575b600080fd5b348015604e57600080fd5b506058600435606a565b60408051918252519081900360200190f35b600101905600a165627a7a72305820ea09447c835e5eb442e1a85e271b0ae6decf8551aa73948ab6b53e8dd1fa0dca0029").unwrap();

        // deploy once
        let (hash, contract) = client.create_contract(contract_code.clone(), &U256::zero());
        let receipt = get_receipt(&hash, &mut dummy_ctx()).unwrap().unwrap();
        let status = receipt.status_code.unwrap();
        assert_eq!(status, 1 as u64);

        // deploy again
        let (hash, contract) = client.create_contract(contract_code.clone(), &U256::zero());
        let receipt = get_receipt(&hash, &mut dummy_ctx()).unwrap().unwrap();
        let status = receipt.status_code.unwrap();
        assert_eq!(status, 1 as u64);
    }

    #[test]
    fn test_signature_verification() {
        let client = CLIENT.lock().unwrap();

        let bad_sig = EthcoreTransaction {
            action: Action::Create,
            nonce: get_account_nonce(&client.keypair.address(), &mut dummy_ctx()).unwrap(),
            gas_price: U256::from(0),
            gas: U256::from(1000000),
            value: U256::from(0),
            data: vec![],
        }.fake_sign(client.keypair.address());
        let bad_result =
            execute_raw_transaction(&rlp::encode(&bad_sig).into_vec(), &mut dummy_ctx())
                .unwrap()
                .hash;
        let good_sig = EthcoreTransaction {
            action: Action::Create,
            nonce: get_account_nonce(&client.keypair.address(), &mut dummy_ctx()).unwrap(),
            gas_price: U256::from(0),
            gas: U256::from(1000000),
            value: U256::from(0),
            data: vec![],
        }.sign(client.keypair.secret(), None);
        let good_result =
            execute_raw_transaction(&rlp::encode(&good_sig).into_vec(), &mut dummy_ctx())
                .unwrap()
                .hash;
        assert!(bad_result.is_err());
        assert!(good_result.is_ok());
    }

    #[test]
    fn test_last_hashes() {
        use state::Cache;

        let mut client = CLIENT.lock().unwrap();

        // ensure that we have >256 blocks
        for i in 0..260 {
            client.create_contract(vec![], &U256::zero());
        }

        // get last_hashes from latest block
        with_batch_handler(|ctx| {
            let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();

            let last_hashes = ectx.cache
                .last_hashes(&ectx.cache.best_block_header().hash());

            assert_eq!(last_hashes.len(), 256);
            assert_eq!(
                last_hashes[1],
                ectx.cache
                    .block_hash(ectx.cache.get_latest_block_number() - 1)
                    .unwrap()
            );
        });
    }

    #[test]
    fn test_cache_invalidation() {
        let mut client = CLIENT.lock().unwrap();

        // Perform initial transaction to get a valid state root.
        let code = hex::decode("3331600055").unwrap(); // SSTORE(0x0, BALANCE(CALLER()))
        let (_, address_1) = client.create_contract(code.clone(), &U256::from(42));
        let state_root_1 = DatabaseHandle::instance().get_root_hash();

        // Perform another transaction to get another state root.
        let (_, address_2) = client.create_contract(code, &U256::from(21));

        // Ensure both contracts exist.
        let best_block = with_batch_handler(|ctx| {
            assert_eq!(get_account_balance(&address_1, ctx), Ok(U256::from(42)));
            assert_eq!(get_account_balance(&address_2, ctx), Ok(U256::from(21)));
            let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();
            ectx.cache.best_block_header().number()
        });

        // Simulate batch rolling back.
        DatabaseHandle::instance()
            .set_root_hash(state_root_1)
            .unwrap();

        // Ensure cache is invalidated correctly.
        with_batch_handler(|ctx| {
            assert_eq!(get_account_balance(&address_1, ctx), Ok(U256::from(42)));
            assert_eq!(get_account_balance(&address_2, ctx), Ok(U256::zero()));
            let ectx = ctx.runtime.downcast_mut::<EthereumContext>().unwrap();
            assert_eq!(best_block, ectx.cache.best_block_header().number() + 1)
        });
    }
}
