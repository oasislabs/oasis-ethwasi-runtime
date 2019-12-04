//! Storage wrappers.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use ethcore;
use failure::{format_err, Fallible};
use io_context::Context;
use ekiden_runtime::storage::{KeyValue, StorageContext};

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
        StorageContext::with_current(|mkvs, _untrusted_local| {
            mkvs.get(Context::create_child(&self.ctx), key)
        })
    }

    fn insert(&mut self, key: &[u8], value: &[u8]) -> Option<Vec<u8>> {
        StorageContext::with_current(|mkvs, _untrusted_local| {
            mkvs.insert(Context::create_child(&self.ctx), key, value)
        })
    }

    fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        StorageContext::with_current(|mkvs, _untrusted_local| {
            mkvs.remove(Context::create_child(&self.ctx), key)
        })
    }

    fn boxed_clone(&self) -> Box<dyn ethcore::mkvs::MKVS> {
        Box::new(ThreadLocalMKVS {
            ctx: self.ctx.clone(),
        })
    }
}

/// In-memory trivial key/value storage.
pub struct MemoryKeyValue(Mutex<HashMap<Vec<u8>, Vec<u8>>>);

impl MemoryKeyValue {
    pub fn new() -> Self {
        MemoryKeyValue(Mutex::new(HashMap::new()))
    }
}

impl KeyValue for MemoryKeyValue {
    fn get(&self, key: Vec<u8>) -> Fallible<Vec<u8>> {
        self.0
            .lock()
            .unwrap()
            .get(&key)
            .cloned()
            .ok_or(format_err!("not found"))
    }

    fn insert(&self, key: Vec<u8>, value: Vec<u8>) -> Fallible<()> {
        self.0.lock().unwrap().insert(key, value);
        Ok(())
    }
}
