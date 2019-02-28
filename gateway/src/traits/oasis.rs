//! Oasis rpc interface.

use ethereum_types::Address;
use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_macros::Trailing;

use parity_rpc::v1::types::{BlockNumber, Bytes, CallRequest, H160};

build_rpc_trait! {
    pub trait Oasis {
        type Metadata;
        /// Returns the public key of a contract, given its address.
        #[rpc(name = "oasis_getPublicKey")]
        fn public_key(&self, Address) -> Result<Option<RpcPublicKeyPayload>>;
        /// Executes a new message call without creating a transaction on chain.
        /// Returns the return value of the executed contract, encrypted with
        /// the user's public key.
        #[rpc(meta, name = "oasis_call_enc")]
        fn call_enc(
            &self,
            Self::Metadata,
            CallRequest,
            Trailing<BlockNumber>
        ) -> BoxFuture<Bytes>;
        /// Get expiration timestamp for a contract.
        /// The value is a Unix timestamp (seconds since the epoch).
        #[rpc(name = "oasis_getExpiry")]
        fn get_expiry(&self, H160, Trailing<BlockNumber>) -> BoxFuture<u64>;
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcPublicKeyPayload {
    /// Public key of the contract
    pub public_key: Bytes,
    /// Time at which the key expires.
    pub timestamp: u64,
    /// Signature from the key manager authenticating the public key,
    /// i.e., Sign(ssk, (pk, t).
    pub signature: Bytes,
}
