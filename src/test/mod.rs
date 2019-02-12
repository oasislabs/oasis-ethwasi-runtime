//! Module for common utilities used in runtime tests.

mod client;
pub use self::client::Client;

use super::EthereumBatchHandler;
use ekiden_common::futures::Future;
use ekiden_core::bytes::H256 as EkidenH256;
use ekiden_roothash_base::header::Header;
use ekiden_storage_base::StorageBackend;
use ekiden_storage_dummy::DummyStorageBackend;
use ekiden_trusted::{
    db::{Database, DatabaseHandle},
    runtime::dispatcher::{BatchHandler, RuntimeCallContext},
};
use std::sync::Arc;

use *;

lazy_static! {
    // Global dummy storage used in tests.
    static ref STORAGE: Arc<StorageBackend> = Arc::new(DummyStorageBackend::new());

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
