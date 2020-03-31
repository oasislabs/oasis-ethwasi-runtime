//! Ethereum block creation.
use std::{collections::HashSet, sync::Arc};

use ethcore::{self, state::State, vm::EnvInfo};
use ethereum_types::{H256, U256};
use io_context::Context as IoContext;
use oasis_core_keymanager_client::KeyManagerClient;
use oasis_core_runtime::{
    common::logger::get_logger, runtime_context, transaction::Context as TxnContext,
};
use oasis_runtime_common::{
    confidential::ConfidentialCtx, genesis, parity::NullBackend, storage::ThreadLocalMKVS,
};
use slog::{info, Logger};

pub struct BlockContext {
    /// Logger.
    pub logger: Logger,
    /// Ethereum state for the current batch.
    pub state: State<NullBackend>,
    /// Environment info for the current batch.
    pub env_info: EnvInfo,
    /// Set of executed transactions.
    pub transaction_set: HashSet<H256>,
}

/// Oasis runtime batch handler.
pub struct OasisBatchHandler {
    key_manager: Arc<dyn KeyManagerClient>,
}

impl OasisBatchHandler {
    pub fn new(key_manager: Arc<dyn KeyManagerClient>) -> Self {
        Self { key_manager }
    }

    pub fn start_batch(&self, ctx: &mut TxnContext) {
        let logger = get_logger("ethereum/block");

        info!(logger, "Computing new block"; "round" => ctx.header.round + 1);

        // Initialize Ethereum state access functions.
        let state = State::from_existing(
            Box::new(ThreadLocalMKVS::new(IoContext::create_child(&ctx.io_ctx))),
            NullBackend,
            U256::zero(),       /* account_start_nonce */
            Default::default(), /* factories */
            Some(Box::new(ConfidentialCtx::new(
                ctx.header.previous_hash.as_ref().into(),
                ctx.io_ctx.clone(),
                self.key_manager.clone(),
            ))),
        )
        .expect("state initialization must succeed");

        // Initialize Ethereum environment information.
        let env_info = EnvInfo {
            number: ctx.header.round + 1,
            author: Default::default(),
            timestamp: ctx.header.timestamp,
            difficulty: Default::default(),
            gas_limit: *genesis::GAS_LIMIT,
            // TODO: Get 256 last_hashes.
            last_hashes: Arc::new(vec![ctx.header.previous_hash.as_ref().into()]),
            gas_used: Default::default(),
        };

        ctx.runtime = Box::new(BlockContext {
            logger,
            state,
            env_info,
            transaction_set: HashSet::new(),
        });
    }

    pub fn end_batch(&self, ctx: &mut TxnContext) {
        let ectx = runtime_context!(ctx, BlockContext);

        info!(ectx.logger, "Commiting state into storage");
        ectx.state.commit().expect("state commit must succeed");
        info!(ectx.logger, "Block finalized");
    }
}
