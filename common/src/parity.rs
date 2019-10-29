//! Common parity helpers.
use std::sync::Arc;

use ethcore::{self, state::Account};
use ethereum_types::{Address, H256};
use hashdb::HashDB;

/// Null backend for parity state.
///
/// This backend is never actually used as a HashDB because Parity
/// has been updated to use our MKVS for storage.
pub struct NullBackend;

impl ethcore::state::backend::Backend for NullBackend {
    fn as_hashdb(&self) -> &dyn HashDB {
        unimplemented!("HashDB should never be used");
    }

    fn as_hashdb_mut(&mut self) -> &mut dyn HashDB {
        unimplemented!("HashDB should never be used");
    }

    fn add_to_account_cache(&mut self, _: Address, _: Option<Account>, _: bool) {}

    fn cache_code(&self, _: H256, _: Arc<Vec<u8>>) {}

    fn get_cached_account(&self, _: &Address) -> Option<Option<Account>> {
        None
    }

    fn get_cached<F, U>(&self, _: &Address, _: F) -> Option<U>
    where
        F: FnOnce(Option<&mut Account>) -> U,
    {
        None
    }

    fn get_cached_code(&self, _: &H256) -> Option<Arc<Vec<u8>>> {
        None
    }
    fn note_non_null_account(&self, _: &Address) {}
    fn is_known_null(&self, _: &Address) -> bool {
        false
    }
}
