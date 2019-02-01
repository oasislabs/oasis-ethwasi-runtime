//! Oasis rpc interface.

use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_macros::Trailing;

use parity_rpc::v1::types::{BlockNumber, H160, H256};

build_rpc_trait! {
    pub trait Oasis {
        /// Get expiration timestamp for a contract.
        /// The value is a Unix timestamp (seconds since the epoch). A special value of 0 indicates
        /// that the contract's storage does not expire.
        #[rpc(name = "oasis_getExpiry")]
        fn get_expiry(&self, H160, Trailing<BlockNumber>) -> BoxFuture<u64>;

        /// Request data from storage.
        #[rpc(name = "oasis_fetchBytes")]
        fn fetch_bytes(&self, H256) -> Result<Vec<u8>>;

        /// Store data in global storage.
        #[rpc(name = "oasis_storeBytes")]
        fn store_bytes(&self, Vec<u8>, u64) -> Result<H256>;
    }
}
