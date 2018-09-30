use ekiden_common::bytes::B512;
use ekiden_keymanager_common::PublicKeyType;
use ethereum_types::Address;
use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_macros::Trailing;
use parity_rpc::v1::types::{BlockNumber, Bytes, CallRequest, H256};

build_rpc_trait! {
    pub trait Confidential {
        type Metadata;
        /// Returns the public key of a contract, given its address.
        #[rpc(name = "confidential_getPublicKey")]
        fn public_key(&self, Address) -> Result<PublicKeyResult>;
        /// Executes a new message call without creating a transaction on chain.
        /// Returns the return value of the executed contract, encrypted with
        /// the user's public key.
        #[rpc(meta, name = "confidential_call_enc")]
        fn call_enc(
            &self,
            Self::Metadata,
            CallRequest,
            Trailing<BlockNumber>
        ) -> BoxFuture<Bytes>;
        /// Executes a raw transaction, where the data field is encrypted.
        #[rpc(name = "confidential_sendRawTransaction")]
        fn send_raw_transaction(&self, Bytes) -> Result<H256>;
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PublicKeyResult {
    /// Public key of the contract
    pub public_key: PublicKeyType,
    /// Time at which the key was issued
    pub timestamp: u64,
    /// Signature from the key manager authenticating the public key,
    /// i.e., Sign(ssk, (pk, t).
    pub signature: B512,
}
