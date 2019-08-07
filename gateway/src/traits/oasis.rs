//! Oasis RPC interface.
use ethereum_types::Address;
use jsonrpc_core::BoxFuture;
use jsonrpc_macros::Trailing;

use parity_rpc::v1::types::{BlockNumber, Bytes, H160, H256, U64};

build_rpc_trait! {
    pub trait Oasis {
        type Metadata;
        /// Returns the public key of a contract, given its address.
        #[rpc(name = "oasis_getPublicKey")]
        fn public_key(&self, Address) -> BoxFuture<Option<RpcPublicKeyPayload>>;

        /// Gets the expiration timestamp for a contract.
        /// The value is a Unix timestamp (seconds since the epoch).
        #[rpc(name = "oasis_getExpiry")]
        fn get_expiry(&self, H160, Trailing<BlockNumber>) -> BoxFuture<u64>;

        /// Sends a signed transaction, and returns the transaction hash,
        /// status code and return value.
        #[rpc(name = "oasis_invoke")]
        fn invoke(&self, Bytes) -> BoxFuture<RpcExecutionPayload>;
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcExecutionPayload {
    /// Transaction hash.
    #[serde(rename = "transactionHash")]
    pub transaction_hash: H256,
    /// Status code.
    #[serde(rename = "status")]
    pub status_code: U64,
    /// Return value.
    pub output: Bytes,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcPublicKeyPayload {
    /// Public key of the contract.
    pub public_key: Bytes,
    /// Checksum of the key manager state.
    pub checksum: Bytes,
    /// Signature from the key manager authenticating the public key,
    /// i.e., Sign(ssk, (pk, t).
    pub signature: Bytes,
}
