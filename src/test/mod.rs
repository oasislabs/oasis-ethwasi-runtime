//! Module for common utilities used in runtime tests.

mod client;
pub use self::client::Client;

use super::EthereumBatchHandler;
use ekiden_common::futures::{BoxFuture, BoxStream, Future};
use ekiden_core::bytes::H256 as EkidenH256;
use ekiden_roothash_base::header::Header;
use ekiden_storage_base::{InsertOptions, StorageBackend};
use ekiden_storage_dummy::DummyStorageBackend;
use ekiden_trusted::{
    db::{Database, DatabaseHandle},
    runtime::dispatcher::{BatchHandler, RuntimeCallContext},
};
use std::sync::Arc;

use *;

struct InstrumentedStorage {
    delegate: DummyStorageBackend,
}

impl InstrumentedStorage {
    fn new() -> Self {
        Self {
            delegate: DummyStorageBackend::new(),
        }
    }
}

impl StorageBackend for InstrumentedStorage {
    fn get(&self, key: EkidenH256) -> BoxFuture<Vec<u8>> {
        storagestudy::dump("get");
        self.delegate.get(key)
    }

    fn get_batch(&self, keys: Vec<EkidenH256>) -> BoxFuture<Vec<Option<Vec<u8>>>> {
        unimplemented!("get_batch not available in UntrustedStorageBackend")
    }

    fn insert(&self, value: Vec<u8>, expiry: u64, opts: InsertOptions) -> BoxFuture<()> {
        storagestudy::dump("set");
        self.delegate.insert(value, expiry, opts)
    }

    fn insert_batch(&self, values: Vec<(Vec<u8>, u64)>, opts: InsertOptions) -> BoxFuture<()> {
        unimplemented!("insert_batch not available in UntrustedStorageBackend")
    }

    fn get_keys(&self) -> BoxStream<(EkidenH256, u64)> {
        unimplemented!("get_keys not available in UntrustedStorageBackend")
    }
}

lazy_static! {
    // Global dummy storage used in tests.
    static ref STORAGE: Arc<StorageBackend> = Arc::new(InstrumentedStorage::new());

    // Genesis block state root as Ekiden H256.
    static ref GENESIS_STATE_ROOT: EkidenH256 =
        EkidenH256::from_slice(&super::evm::SPEC.state_root().to_vec());
}

pub fn dummy_ctx() -> RuntimeCallContext {
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

pub fn with_batch_handler<F, R>(timestamp: u64, f: F) -> R
where
    F: FnOnce(&mut RuntimeCallContext) -> R,
{
    let root_hash = DatabaseHandle::instance().get_root_hash();
    let mut ctx = RuntimeCallContext::new(Header {
        timestamp: timestamp,
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

/// Sends a transaction onchain that updates the blockchain.
pub fn send_in_batch_keypair(
    ctx: &mut RuntimeCallContext,
    keypair: &ethkey::KeyPair,
    gas_price: U256,
    gas_limit: U256,
    contract: Option<&Address>,
    data: Vec<u8>,
    value: &U256,
) -> H256 {
    let tx = EthcoreTransaction {
        action: if contract == None {
            Action::Create
        } else {
            Action::Call(*contract.unwrap())
        },
        nonce: get_account_nonce(&keypair.address(), ctx).unwrap(),
        gas_price,
        gas: gas_limit,
        value: *value,
        data: data,
    }
    .sign(&keypair.secret(), None);

    let raw = rlp::encode(&tx);
    execute_raw_transaction(&raw.into_vec(), ctx)
        .unwrap()
        .hash
        .unwrap()
}
