//! Storage wrappers.
use std::sync::Arc;

use ekiden_runtime::storage::StorageContext;
use ethcore;
use io_context::Context;

/// MKVS implementation which uses the thread-local MKVS provided by
/// the `StorageContext`.
pub struct ThreadLocalMKVS {
    // TODO: The proper way would be to change Parity API to support contexts.
    ctx: Arc<Context>,
}

impl ThreadLocalMKVS {
    pub fn new(ctx: Context) -> Self {
        Self { ctx: ctx.freeze() }
    }
}

impl ethcore::mkvs::MKVS for ThreadLocalMKVS {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        StorageContext::with_current(|_cas, mkvs| mkvs.get(Context::create_child(&self.ctx), key))
    }

    fn insert(&mut self, key: &[u8], value: &[u8]) -> Option<Vec<u8>> {
        StorageContext::with_current(|_cas, mkvs| {
            mkvs.insert(Context::create_child(&self.ctx), key, value)
        })
    }

    fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        StorageContext::with_current(|_cas, mkvs| {
            mkvs.remove(Context::create_child(&self.ctx), key)
        })
    }

    fn boxed_clone(&self) -> Box<ethcore::mkvs::MKVS> {
        Box::new(ThreadLocalMKVS {
            ctx: self.ctx.clone(),
        })
    }
}
