//! Eth rpc interface.
use jsonrpc_core::Result;

use parity_rpc::v1::types::H256;

build_rpc_trait! {
    pub trait Oasis {
        /// Request data from storage.
        #[rpc(name = "oasis_fetchBytes")]
        fn fetch_bytes(&self, H256) -> Result<Vec<u8>>;

        /// Store data in global storage.
        #[rpc(name = "oasis_storeBytes")]
        fn store_bytes(&self, Vec<u8>, u64) -> Result<H256>;
    }
}
