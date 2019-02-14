//! Oasis rpc interface.

use jsonrpc_core::{BoxFuture};
use jsonrpc_macros::Trailing;

use parity_rpc::v1::types::{BlockNumber, H160};

build_rpc_trait! {
    pub trait Oasis {
        /// Get expiration timestamp for a contract.
        /// The value is a Unix timestamp (seconds since the epoch). A special value of 0 indicates
        /// that the contract's storage does not expire.
        #[rpc(name = "oasis_getExpiry")]
        fn get_expiry(&self, H160, Trailing<BlockNumber>) -> BoxFuture<u64>;
    }
}
