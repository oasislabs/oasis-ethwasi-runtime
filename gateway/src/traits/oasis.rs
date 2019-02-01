//! Oasis rpc interface.

use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_macros::Trailing;

use parity_rpc::v1::types::{BlockNumber, H160, H256};

build_rpc_trait! {
    pub trait Oasis {
        /// Get storage expiration timestamp for an address.
        /// The value is a Unix timestamp (seconds since the epoch). A special value of 0 indicates
        /// that the contract's storage does not expire.
        #[rpc(name = "oasis_getStorageExpiry")]
        fn get_storage_expiry(&self, H160, Trailing<BlockNumber>) -> BoxFuture<u64>;
    }
}
