//! Eth rpc interface.
use jsonrpc_core:::Result;
use jsonrpc_macros::Trailing;

use parity_rpc::v1::types::{H256, U256, Bytes};

build_rpc_trait! {
    pub trait Oasis {
        /// Request data from storage.
        #[rpc(name = "oasis_requestBytes")]
        fn request_bytes(&self, H256) -> Result<String>;

        /// Store data in global storage.
        #[rpc(name = "oasis_storeBytes")]
        fn store_bytes(&self, String, U64) -> Result<H256>;
    }
}
