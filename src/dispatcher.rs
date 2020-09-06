use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::{anyhow, bail, Context as AnyContext, Error as AnyError, Result};
use ethcore::transaction::SignedTransaction;
#[cfg(feature = "prefetch")]
use ethcore::{
    state::{MKVS_KEY_CODE, MKVS_KEY_METADATA},
    transaction::Action,
};
use serde_bytes::ByteBuf;
use thiserror::Error;

use oasis_core_keymanager_client::KeyManagerClient;
#[cfg(feature = "prefetch")]
use oasis_core_runtime::storage::mkvs::Prefix;
use oasis_core_runtime::{
    common::{cbor, crypto::hash::Hash, roothash::Message as RoothashMessage},
    transaction::{
        context::Context,
        dispatcher::{CheckOnlySuccess, Dispatcher as TxnDispatcher},
        tags::Tags,
        types::*,
    },
};

use super::{
    block::OasisBatchHandler,
    methods::{check, execute},
};

use oasis_ethwasi_runtime_api as api;

/// Dispatch error.
#[derive(Error, Debug)]
enum DispatchError {
    #[error("method not found: {method}")]
    MethodNotFound { method: String },
}

pub struct DecodedCall {
    pub transaction: SignedTransaction,
}

pub struct Dispatcher {
    /// Registered batch handler.
    batch_handler: OasisBatchHandler,
    /// Abort batch flag.
    /// Aborting not implemented.
    abort_batch: Option<Arc<AtomicBool>>,
}

impl Dispatcher {
    /// Create a new runtime method dispatcher instance.
    pub fn new(key_manager: Arc<dyn KeyManagerClient>) -> Dispatcher {
        Dispatcher {
            batch_handler: OasisBatchHandler::new(key_manager),
            abort_batch: None,
        }
    }

    fn decode_transaction(&self, call: &[u8], ctx: &mut Context) -> Result<DecodedCall> {
        let call: TxnCall = cbor::from_slice(call).context("unable to parse call")?;

        if call.method != api::METHOD_TX {
            return Err(DispatchError::MethodNotFound {
                method: call.method,
            }
            .into());
        }

        let call_args: ByteBuf =
            cbor::from_value(call.args).context("unable to parse call arguments")?;
        let signed_transaction = check::tx(&call_args, ctx)?;

        Ok(DecodedCall {
            transaction: signed_transaction,
        })
    }

    fn encode_response(&self, call: &DecodedCall, ctx: &mut Context) -> Result<Vec<u8>> {
        let response = execute::tx(call, ctx)?;
        let response = TxnOutput::Success(cbor::to_value(response));
        Ok(cbor::to_vec(&response))
    }

    fn serialize_error(&self, err: &AnyError) -> Vec<u8> {
        let txn_output = match err.downcast_ref::<CheckOnlySuccess>() {
            Some(check_result) => TxnOutput::Success(cbor::to_value(check_result.0.clone())),
            None => TxnOutput::Error(format!("{}", err)),
        };
        cbor::to_vec(&txn_output)
    }
}

impl TxnDispatcher for Dispatcher {
    fn dispatch_batch(
        &self,
        batch: &TxnBatch,
        mut ctx: Context,
    ) -> Result<(TxnBatch, Vec<Tags>, Vec<RoothashMessage>)> {
        // Invoke start batch handler.
        self.batch_handler.start_batch(&mut ctx);

        #[cfg(feature = "prefetch")]
        let mut prefixes: Vec<Prefix> = Vec::new();

        // Decode and check transactions in this batch.
        let calls: Vec<Result<DecodedCall>> = batch
            .iter()
            .map(|call| {
                if self
                    .abort_batch
                    .as_ref()
                    .map(|b| b.load(Ordering::SeqCst))
                    .unwrap_or(false)
                {
                    bail!("batch aborted");
                }
                let tx = self.decode_transaction(call, &mut ctx)?;

                #[cfg(feature = "prefetch")]
                {
                    if !ctx.check_only {
                        if let Action::Call(receiver) = (**tx.transaction).action {
                            let mut account_code: Vec<u8> = receiver.to_vec();
                            account_code.extend_from_slice(MKVS_KEY_CODE);
                            prefixes.push(account_code.into());

                            let mut account_meta: Vec<u8> = receiver.to_vec();
                            account_meta.extend_from_slice(MKVS_KEY_METADATA);
                            prefixes.push(Prefix::from(account_meta));
                        }

                        let mut account_meta: Vec<u8> = tx.transaction.sender().to_vec();
                        account_meta.extend_from_slice(MKVS_KEY_METADATA);
                        prefixes.push(Prefix::from(account_meta));
                    }
                }

                Ok(tx)
            })
            .collect();

        #[cfg(feature = "prefetch")]
        {
            if !ctx.check_only {
                use io_context::Context as IoContext;
                use oasis_core_runtime::storage::StorageContext;

                prefixes.sort_unstable();
                prefixes.dedup();

                StorageContext::with_current(|mkvs, _untrusted_local| {
                    prefixes.drain_filter(|key| {
                        mkvs.cache_contains_key(IoContext::create_child(&ctx.io_ctx), key)
                    });

                    if prefixes.len() > 0 {
                        mkvs.prefetch_prefixes(
                            IoContext::create_child(&ctx.io_ctx),
                            &prefixes,
                            10_000, /* limit */
                        )
                    }
                });
            }
        }

        // Process batch.
        let outputs = TxnBatch::new(
            calls
                .iter()
                .map(|call| {
                    if self
                        .abort_batch
                        .as_ref()
                        .map(|b| b.load(Ordering::SeqCst))
                        .unwrap_or(false)
                    {
                        return self.serialize_error(&anyhow!("batch aborted"));
                    }
                    match call {
                        Ok(call) => self
                            .encode_response(call, &mut ctx)
                            .unwrap_or_else(|err| self.serialize_error(&err)),
                        Err(err) => self.serialize_error(err),
                    }
                })
                .collect(),
        );

        // Invoke end batch handler.
        self.batch_handler.end_batch(&mut ctx);

        let (tags, roothash_messages) = ctx.close();
        Ok((outputs, tags, roothash_messages))
    }

    fn finalize(&self, _new_storage_root: Hash) {}

    /// Configure abort batch flag.
    fn set_abort_batch_flag(&mut self, abort_batch: Arc<AtomicBool>) {
        self.abort_batch = Some(abort_batch);
    }
}
