//! Ethereum runtime.
#[cfg(feature = "test")]
extern crate byteorder;
extern crate bytes;
extern crate ekiden_client;
extern crate ekiden_keymanager_client;
extern crate ekiden_runtime;
#[cfg(feature = "test")]
extern crate elastic_array;
extern crate ethcore;
extern crate ethereum_types;
#[cfg(feature = "test")]
extern crate ethkey;
extern crate failure;
extern crate io_context;
extern crate lazy_static;
extern crate runtime_ethereum_api;
extern crate runtime_ethereum_common;
#[cfg(feature = "test")]
#[macro_use]
extern crate serde_json;
extern crate slog;

pub mod cache;
pub mod execution;
pub mod genesis;
pub mod methods;
pub mod util;

#[cfg(feature = "test")]
pub mod test;

use std::sync::Arc;

use ekiden_runtime::{
    common::logger::get_logger,
    runtime_context_move,
    transaction::{dispatcher::BatchHandler, Context as TxnContext},
};
use ethcore::block::{IsBlock, OpenBlock};
use slog::Logger;

use self::cache::Cache;

// Include key manager enclave hash.
include!(concat!(env!("OUT_DIR"), "/km_enclave_hash.rs"));

struct Context<'a> {
    /// Logger.
    logger: Logger,
    /// Parity blockchain cache.
    cache: Arc<Cache>,
    /// Whether to force emitting a block.
    force_emit_block: bool,
    /// An open block.
    open_block: OpenBlock<'a>,
}

/// Ethereum runtime batch handler.
pub struct EthereumBatchHandler {
    cache: Arc<Cache>,
}

impl EthereumBatchHandler {
    /// Create a new ethereum runtime batch handler.
    pub fn new(cache: Arc<Cache>) -> Self {
        Self { cache }
    }
}

impl BatchHandler for EthereumBatchHandler {
    fn start_batch(&self, ctx: &mut TxnContext) {
        self.cache
            .init(ctx.header.state_root)
            .expect("blockchain cache init must succeed");

        ctx.runtime = Box::new(Context {
            logger: get_logger("ethereum/batch"),
            cache: self.cache.clone(),
            force_emit_block: false,
            open_block: {
                let mut block = self.cache.new_block(ctx.io_ctx.clone()).unwrap();
                block.set_timestamp(ctx.header.timestamp);
                block
            },
        });
    }

    fn end_batch(&self, ctx: TxnContext) {
        let rctx = runtime_context_move!(ctx, Context);

        // Finalize the block if it contains any transactions.
        if !rctx.open_block.transactions().is_empty() || rctx.force_emit_block {
            self.cache
                .add_block(rctx.open_block.close_and_lock())
                .expect("blockchain add_block must succeed");
        }
    }
}
