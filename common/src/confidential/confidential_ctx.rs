#![deny(warnings)]
use std::sync::Arc;

use ekiden_keymanager_client::{ContractId, ContractKey, KeyManagerClient, PublicKey};
use ekiden_runtime::{
    common::crypto::mrae::{
        nonce::{Nonce, NONCE_SIZE},
        sivaessha2::{SivAesSha2, KEY_SIZE},
    },
    executor::Executor,
};
use ethcore::state::ConfidentialCtx as EthConfidentialCtx;
use ethereum_types::Address;
use failure::{format_err, Fallible, ResultExt};
use io_context::Context;
use keccak_hash::keccak;

use super::crypto;

/// Facade for the underlying confidential contract services to be injected into
/// the parity state. Manages the confidential state--i.e., encryption keys and
/// nonce to use--to be encrypting under for a *single* transaction. Each
/// transaction for a confidential contract should have it's own ConfidentialCtx
/// that is closed at the end of the transaction's execution.
pub struct ConfidentialCtx {
    /// The peer public key used for encryption. This should not change for an
    /// open confidential context. This is implicitly set by the `open` method.
    pub peer_public_key: Option<PublicKey>,
    /// The contract address and keys used for encryption. These keys may be
    /// swapped in an open confidential context, facilitating a confidential
    /// context switch to encrypt for the *same user* but under a different
    /// contract.
    pub contract_key: Option<ContractKey>,
    /// The next nonce to use when encrypting a message to `peer_public_key`.
    /// This starts at the nonce+1 given by the `encrypted_tx_data` param in the
    /// `open_tx_data` fn. Then, throughout the context, is incremented each
    /// time a message is encrypted to the `peer_public_key`.
    pub next_nonce: Option<Nonce>,
    /// Key manager client.
    pub key_manager: Arc<KeyManagerClient>,
    /// IO context (needed for the key manager client).
    pub io_ctx: Arc<Context>,
}

impl ConfidentialCtx {
    pub fn new(io_ctx: Arc<Context>, key_manager: Arc<KeyManagerClient>) -> Self {
        Self {
            peer_public_key: None,
            contract_key: None,
            next_nonce: None,
            key_manager,
            io_ctx,
        }
    }

    pub fn open_tx_data(&mut self, encrypted_tx_data: Vec<u8>) -> Fallible<Vec<u8>> {
        if self.contract_key.is_none() {
            return Err(format_err!("The confidential context must have a contract key when opening encrypted transaction data"));
        }
        let contract_secret_key = self.contract_key.as_ref().unwrap().input_keypair.get_sk();

        let decryption = crypto::decrypt(Some(encrypted_tx_data), contract_secret_key)
            .with_context(|err| format!("Unable to decrypt transaction data: {}", err))?;
        self.peer_public_key = Some(decryption.peer_public_key);

        let mut nonce = decryption.nonce;
        nonce.increment()?;
        self.next_nonce = Some(nonce);

        Ok(decryption.plaintext)
    }

    pub fn decrypt(&self, encrypted_tx_data: Vec<u8>) -> Fallible<Vec<u8>> {
        let contract_secret_key = self.contract_key.as_ref().unwrap().input_keypair.get_sk();
        let decryption = crypto::decrypt(Some(encrypted_tx_data), contract_secret_key)?;

        Ok(decryption.plaintext)
    }
}

impl EthConfidentialCtx for ConfidentialCtx {
    fn open(&mut self, contract: Address, encrypted_tx_data: Option<Vec<u8>>) -> Fallible<Vec<u8>> {
        if self.is_open() {
            return Err(format_err!(
                "can't open a confidential context that's already open"
            ));
        }

        let contract_id = ContractId::from(&keccak(contract.to_vec())[..]);
        let contract_key = Executor::with_current(|executor| {
            executor
                .block_on(
                    self.key_manager
                        .get_or_create_keys(Context::create_child(&self.io_ctx), contract_id),
                )
                .context("failed to get or create keys")
        })?;

        self.contract_key = Some(contract_key);

        if encrypted_tx_data.is_some() {
            let data = self.open_tx_data(encrypted_tx_data.unwrap());
            if data.is_err() {
                self.close();
            }
            data
        } else {
            Ok(vec![])
        }
    }

    fn is_open(&self) -> bool {
        self.contract_key.is_some()
    }

    fn close(&mut self) {
        self.peer_public_key = None;
        self.contract_key = None;
        self.next_nonce = None;
    }

    fn encrypt(&mut self, data: Vec<u8>) -> Fallible<Vec<u8>> {
        if self.peer_public_key.is_none()
            || self.contract_key.is_none()
            || self.next_nonce.is_none()
        {
            return Err(format_err!(
                "must have key pair of a contract and peer and a next nonce"
            ));
        }

        let contract_key = &self.contract_key.as_ref().unwrap();
        let contract_pk = contract_key.input_keypair.get_pk();
        let contract_sk = contract_key.input_keypair.get_sk();

        let encrypted_payload = crypto::encrypt(
            data,
            self.next_nonce.clone().unwrap(),
            self.peer_public_key.clone().unwrap(),
            contract_pk,
            contract_sk,
        );

        // NOTE: Result is only checked after the nonce is incremented.

        self.next_nonce.as_mut().unwrap().increment()?;

        Ok(encrypted_payload?)
    }

    fn encrypt_storage(&self, data: Vec<u8>) -> Fallible<Vec<u8>> {
        let contract_key = &self
            .contract_key
            .as_ref()
            .expect("should always have a contract key to encrypt storage");
        let state_key = contract_key.state_key;

        // TODO: This should not use a fixed nonce.

        let key: Vec<u8> = state_key.as_ref()[..KEY_SIZE].to_vec();
        let nonce: Vec<u8> = state_key.as_ref()[KEY_SIZE..KEY_SIZE + NONCE_SIZE].to_vec();

        let mrae = SivAesSha2::new(key).unwrap();

        Ok(mrae.seal(nonce, data, vec![]).unwrap())
    }

    fn decrypt_storage(&self, data: Vec<u8>) -> Fallible<Vec<u8>> {
        let contract_key = &self
            .contract_key
            .as_ref()
            .expect("should always have a contract key to decrypt storage");
        let state_key = contract_key.state_key;

        // TODO: This should not use a fixed nonce.

        let key: Vec<u8> = state_key.as_ref()[..KEY_SIZE].to_vec();
        let nonce: Vec<u8> = state_key.as_ref()[KEY_SIZE..KEY_SIZE + NONCE_SIZE].to_vec();

        let mrae = SivAesSha2::new(key).unwrap();

        Ok(mrae.open(nonce, data, vec![]).unwrap())
    }

    fn create_long_term_public_key(&mut self, contract: Address) -> Fallible<(Vec<u8>, Vec<u8>)> {
        let contract_id = ContractId::from(&keccak(contract.to_vec())[..]);
        let pk = Executor::with_current(|executor| {
            executor
                .block_on(
                    self.key_manager
                        .get_or_create_keys(Context::create_child(&self.io_ctx), contract_id),
                )
                .context("failed to create keys")?;

            executor
                .block_on(
                    self.key_manager
                        .get_long_term_public_key(Context::create_child(&self.io_ctx), contract_id),
                )
                .context("failed to fetch long term key")?
                .ok_or(format_err!("failed to create keys"))
        })?;

        Ok((pk.key.as_ref().to_vec(), pk.signature.as_ref().to_vec()))
    }

    fn peer(&self) -> Option<Vec<u8>> {
        self.peer_public_key.as_ref().map(|pk| pk.as_ref().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use ekiden_keymanager_client::{self, ContractKey, PrivateKey, PublicKey, StateKey};

    use super::*;

    #[test]
    fn test_open_tx_data_with_no_contract_key() {
        let mut ctx = ConfidentialCtx::new(
            Context::background().freeze(),
            Arc::new(ekiden_keymanager_client::mock::MockClient::new()),
        );
        let res = ctx.open_tx_data(Vec::new());

        assert_eq!(
            &format!("{}", res.err().unwrap()),
            "The confidential context must have a contract key when opening encrypted transaction data"
        );
    }

    #[test]
    fn test_open_tx_data_decrypt_invalid() {
        let peer_public_key = PublicKey::default();
        let public_key = PublicKey::default();
        let private_key = PrivateKey::default();
        let state_key = StateKey::default();
        let contract_key = ContractKey::new(public_key, private_key, state_key);
        let nonce = Nonce::new([0; NONCE_SIZE]);
        let mut ctx = ConfidentialCtx {
            peer_public_key: Some(peer_public_key),
            contract_key: Some(contract_key),
            next_nonce: Some(nonce),
            key_manager: Arc::new(ekiden_keymanager_client::mock::MockClient::new()),
            io_ctx: Context::background().freeze(),
        };

        let res = ctx.open_tx_data(Vec::new());

        assert_eq!(
            &format!("{}", res.err().unwrap()),
            "Unable to decrypt transaction data: invalid nonce or public key"
        );
    }

    #[test]
    fn test_is_open() {
        let peer_public_key = PublicKey::default();
        let public_key = PublicKey::default();
        let private_key = PrivateKey::default();
        let state_key = StateKey::default();
        let contract_key = ContractKey::new(public_key, private_key, state_key);
        let nonce = Nonce::new([0; NONCE_SIZE]);

        assert_eq!(
            ConfidentialCtx {
                peer_public_key: Some(peer_public_key),
                contract_key: Some(contract_key),
                next_nonce: Some(nonce),
                key_manager: Arc::new(ekiden_keymanager_client::mock::MockClient::new()),
                io_ctx: Context::background().freeze(),
            }
            .is_open(),
            true
        );
        assert_eq!(
            ConfidentialCtx {
                peer_public_key: None,
                contract_key: None,
                next_nonce: None,
                key_manager: Arc::new(ekiden_keymanager_client::mock::MockClient::new()),
                io_ctx: Context::background().freeze(),
            }
            .is_open(),
            false
        );
    }

    #[test]
    fn test_open_tx_data_after_close() {
        let peer_public_key = PublicKey::default();
        let public_key = PublicKey::default();
        let private_key = PrivateKey::default();
        let state_key = StateKey::default();
        let contract_key = ContractKey::new(public_key, private_key, state_key);
        let nonce = Nonce::new([0; NONCE_SIZE]);
        let mut ctx = ConfidentialCtx {
            peer_public_key: Some(peer_public_key),
            contract_key: Some(contract_key),
            next_nonce: Some(nonce),
            key_manager: Arc::new(ekiden_keymanager_client::mock::MockClient::new()),
            io_ctx: Context::background().freeze(),
        };

        ctx.close();
        let res = ctx.open_tx_data(Vec::new());

        assert_eq!(
            &format!("{}", res.err().unwrap()),
            "The confidential context must have a contract key when opening encrypted transaction data"
        );
    }
}
