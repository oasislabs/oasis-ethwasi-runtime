#![deny(warnings)]
use std::sync::Arc;

use ethereum_types::{Address, H256};
use failure::ResultExt;
use io_context::Context;
use keccak_hash::keccak;
use oasis_core_keymanager_client::{KeyManagerClient, KeyPair, KeyPairId, PublicKey};
use oasis_core_runtime::{
    common::crypto::{
        hash::Hash,
        mrae::{
            deoxysii::{DeoxysII, KEY_SIZE, TAG_SIZE},
            nonce::{Nonce, NONCE_SIZE, TAG_SIZE as NONCE_TAG_SIZE},
        },
    },
    executor::Executor,
};
use vm::{AuthenticatedPayload, ConfidentialCtx as EthConfidentialCtx, Error, Result};
use zeroize::Zeroize;

use super::crypto;

/// Facade for the underlying confidential contract services to be injected into
/// the parity state. Manages the confidential state--i.e., encryption keys and
/// nonce to use--for a block.
pub struct ConfidentialCtx {
    /// The peer public key used for encryption. This is implicitly set by the
    /// `decrypt_session` method, establishing an encrypted channel to the peer.
    peer_public_key: Option<PublicKey>,
    /// The contract address and keys used for encryption. These keys may be
    /// swapped or set to None in an open confidential context, facilitating
    /// a confidential context switch to encrypt for the *same peer* but under
    /// a different contract.
    contract: Option<(Address, KeyPair)>,
    /// The next nonce to use when encrypting a message to `peer_public_key`.
    /// This starts at the nonce+1 given by the `encrypted_tx_data` param in the
    /// `decrypt_session` fn. Then, throughout the transaction, is incremented each
    /// time a message is encrypted to the `peer_public_key` via `encrypt_session`.
    next_nonce: Option<Nonce>,
    /// True iff the confidential context is activated, i.e., if we've executed
    /// a confidential contract at any point in the call hierarchy.
    activated: bool,
    /// Hash of previous block, used to construct storage encryption nonce.
    prev_block_hash: H256,
    /// Deoxys-II instance used for encrypting and decrypting contract storage.
    d2: Option<DeoxysII>,
    /// The next nonce to use when encrypting a storage value. When we start
    /// executing a confidential transaction, its value is set to
    /// H(prev_block_hash || contract_address)[:11] || 0x00000000. The value is
    /// incremented after each encrypt operation.
    next_storage_nonce: Option<Nonce>,
    /// Key manager client.
    key_manager: Arc<dyn KeyManagerClient>,
    /// IO context (needed for the key manager client).
    io_ctx: Arc<Context>,
}

impl ConfidentialCtx {
    pub fn new(
        prev_block_hash: H256,
        io_ctx: Arc<Context>,
        key_manager: Arc<dyn KeyManagerClient>,
    ) -> Self {
        Self {
            peer_public_key: None,
            contract: None,
            next_nonce: None,
            activated: false,
            d2: None,
            prev_block_hash,
            next_storage_nonce: None,
            key_manager,
            io_ctx,
        }
    }

    /// Constructor to be used for testing only.
    #[cfg(feature = "test")]
    pub fn new_test(
        peer_public_key: Option<PublicKey>,
        contract: Option<(Address, KeyPair)>,
        next_nonce: Option<Nonce>,
        activated: bool,
        prev_block_hash: H256,
        d2: Option<DeoxysII>,
        next_storage_nonce: Option<Nonce>,
        key_manager: Arc<dyn KeyManagerClient>,
        io_ctx: Arc<Context>,
    ) -> Self {
        Self {
            peer_public_key,
            contract,
            next_nonce,
            activated,
            d2,
            prev_block_hash,
            next_storage_nonce,
            key_manager,
            io_ctx,
        }
    }

    pub fn decrypt(&self, encrypted_tx_data: Vec<u8>) -> Result<Vec<u8>> {
        if self.contract.is_none() {
            return Err(Error::Confidential("The confidential context must have a contract key when opening encrypted transaction data".to_string()));
        }
        let contract_secret_key = self.contract.as_ref().unwrap().1.input_keypair.get_sk();
        let decryption = crypto::decrypt(Some(encrypted_tx_data), contract_secret_key)
            .map_err(|err| Error::Confidential(err.to_string()))?;

        Ok(decryption.plaintext)
    }

    fn swap_contract(&mut self, contract: Option<(Address, KeyPair)>) -> Option<Address> {
        let old_contract_address = self.contract.as_ref().map(|c| c.0);
        self.contract = contract;

        // If this is a confidential contract, initialize Deoxys-II instance.
        self.d2 = self.contract.as_ref().map(|c| {
            let state_key = c.1.state_key;
            let mut key = [0u8; KEY_SIZE];
            key.copy_from_slice(&state_key.as_ref()[..KEY_SIZE]);
            let d2 = DeoxysII::new(&key);
            key.zeroize();
            d2
        });

        // Storage encryption nonce <- H(prev_block_hash || address)[:11] || 0x00000000
        self.next_storage_nonce = self.contract.as_ref().map(|c| {
            let mut buffer = self.prev_block_hash.to_vec();
            buffer.extend_from_slice(&c.0);
            let hash = Hash::digest_bytes(&buffer);

            let mut nonce = [0u8; NONCE_SIZE];
            nonce[..NONCE_TAG_SIZE].copy_from_slice(&hash.as_ref()[..NONCE_TAG_SIZE]);

            Nonce::new(nonce)
        });

        old_contract_address
    }
}

impl EthConfidentialCtx for ConfidentialCtx {
    fn is_encrypting(&self) -> bool {
        // Note: self.activated == true and self.contract.is_some() == false when making
        //       a cross-contract-call from confidential -> non-confidential.
        self.activated() && self.contract.is_some()
    }

    fn activated(&self) -> bool {
        self.activated
    }

    /// `contract` is None when making a cross contract call from confidential -> non-confidential.
    /// Otherwise, `contract` is the address of the contract for which we want to start encrypting.
    fn activate(&mut self, contract: Option<Address>) -> Result<Option<Address>> {
        self.activated = true;

        match contract {
            None => Ok(self.swap_contract(None)),
            Some(contract) => {
                let contract_id = KeyPairId::from(&keccak(contract.to_vec())[..]);
                let key_pair =
                    Executor::with_current(|executor| {
                        executor
                            .block_on(self.key_manager.get_or_create_keys(
                                Context::create_child(&self.io_ctx),
                                contract_id,
                            ))
                            .context("failed to get or create keys")
                    })
                    .map_err(|err| Error::Confidential(err.to_string()))?;

                Ok(self.swap_contract(Some((contract, key_pair))))
            }
        }
    }

    fn deactivate(&mut self) {
        self.peer_public_key = None;
        self.contract = None;
        self.next_nonce = None;
        self.activated = false;
        self.d2 = None;
        self.next_storage_nonce = None;
    }

    fn encrypt_session(&mut self, data: Vec<u8>) -> Result<Vec<u8>> {
        if self.peer_public_key.is_none() || self.contract.is_none() || self.next_nonce.is_none() {
            return Err(Error::Confidential(
                "must have key pair of a contract and peer and a next nonce".to_string(),
            ));
        }

        let contract_key = &self.contract.as_ref().unwrap().1;
        let contract_pk = contract_key.input_keypair.get_pk();
        let contract_sk = contract_key.input_keypair.get_sk();

        let encrypted_payload = crypto::encrypt(
            data,
            self.next_nonce.clone().unwrap(),
            self.peer_public_key.clone().unwrap(),
            contract_pk,
            contract_sk,
            vec![],
        )
        .map_err(|err| Error::Confidential(err.to_string()))?;

        self.next_nonce
            .as_mut()
            .unwrap()
            .increment()
            .map_err(|err| Error::Confidential(err.to_string()))?;

        Ok(encrypted_payload)
    }

    fn decrypt_session(&mut self, encrypted_payload: Vec<u8>) -> Result<AuthenticatedPayload> {
        let contract_secret_key = self.contract.as_ref().unwrap().1.input_keypair.get_sk();

        let decryption = crypto::decrypt(Some(encrypted_payload), contract_secret_key)
            .map_err(|err| Error::Confidential(err.to_string()))?;
        self.peer_public_key = Some(decryption.peer_public_key);

        let mut nonce = decryption.nonce;
        nonce
            .increment()
            .map_err(|err| Error::Confidential(err.to_string()))?;
        self.next_nonce = Some(nonce);

        Ok(AuthenticatedPayload {
            decrypted_data: decryption.plaintext,
            additional_data: decryption.aad,
        })
    }

    fn encrypt_storage_key(&self, data: Vec<u8>) -> Result<Vec<u8>> {
        let nonce = [0u8; NONCE_SIZE];
        //^ We're not trying to protect the identity of the key, so a zero nonce is fine.
        //  We could also use AES-ECB, therefore.
        Ok(self
            .d2
            .as_ref()
            .expect("Should always have a Deoxys-II instance to encrypt storage")
            .seal(&nonce, data, vec![]))
    }

    // This implementation of `ConfidentialCtx` ensures key-value mapping integrity by setting the
    // (encrypted) `storage_key` as the AAD.
    fn encrypt_storage_value(&mut self, storage_key: Vec<u8>, data: Vec<u8>) -> Result<Vec<u8>> {
        let mut nonce = [0u8; NONCE_SIZE];
        nonce.copy_from_slice(
            &self
                .next_storage_nonce
                .as_ref()
                .expect("Should always have a storage encryption nonce.")[..NONCE_SIZE],
        );

        let mut ciphertext = self
            .d2
            .as_ref()
            .expect("Should always have a Deoxys-II instance to encrypt storage")
            .seal(&nonce, data, storage_key);
        ciphertext.extend_from_slice(&nonce); // ciphertext || tag || nonce

        self.next_storage_nonce
            .as_mut()
            .unwrap()
            .increment()
            .map_err(|err| Error::Confidential(err.to_string()))?;

        Ok(ciphertext)
    }

    // This implementation of `ConfidentialCtx` ensures key-value mapping integrity by checking the
    // AAD stored with the value against the expected (encrypted) `storage_key`.
    fn decrypt_storage_value(&self, storage_key: Vec<u8>, data: Vec<u8>) -> Result<Vec<u8>> {
        if data.len() < TAG_SIZE + NONCE_SIZE {
            return Err(Error::Confidential("truncated ciphertext".to_string()));
        }

        // Split out the nonce from the tail of ciphertext || tag || nonce.
        let nonce_offset = data.len() - NONCE_SIZE;
        let mut nonce = [0u8; NONCE_SIZE];
        nonce.copy_from_slice(&data[nonce_offset..]);
        let ciphertext = &data[..nonce_offset];

        self.d2
            .as_ref()
            .expect("Should always have a Deoxys-II instance to decrypt storage")
            .open(&nonce, ciphertext.to_vec(), storage_key)
            .map_err(|_| Error::Confidential("invalid storage (decryption) key".to_string()))
    }

    fn peer(&self) -> Option<Vec<u8>> {
        self.peer_public_key.as_ref().map(|pk| pk.as_ref().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use oasis_core_keymanager_client::{self, KeyPair, PrivateKey, PublicKey, StateKey};

    use super::*;

    #[test]
    fn test_decrypt_with_no_contract_key() {
        let ctx = ConfidentialCtx::new(
            H256::default(),
            Context::background().freeze(),
            Arc::new(oasis_core_keymanager_client::mock::MockClient::new()),
        );
        let res = ctx.decrypt(Vec::new());

        assert_eq!(
            &format!("{}", res.err().unwrap()),
            "Confidential error: The confidential context must have a contract key when opening encrypted transaction data"
        );
    }

    #[test]
    fn test_decrypt_invalid() {
        let peer_public_key = PublicKey::default();
        let public_key = PublicKey::default();
        let private_key = PrivateKey::default();
        let state_key = StateKey::default();
        let contract_key = KeyPair::new(public_key, private_key, state_key, vec![]);
        let nonce = Nonce::new([0; NONCE_SIZE]);
        let address = Address::default();
        let ctx = ConfidentialCtx {
            peer_public_key: Some(peer_public_key),
            contract: Some((address, contract_key)),
            next_nonce: Some(nonce.clone()),
            prev_block_hash: H256::default(),
            next_storage_nonce: Some(nonce),
            // No storage encryption, so don't need a Deoxys-II instance.
            d2: None,
            key_manager: Arc::new(oasis_core_keymanager_client::mock::MockClient::new()),
            io_ctx: Context::background().freeze(),
            activated: true,
        };

        let res = ctx.decrypt(Vec::new());

        assert_eq!(
            &format!("{}", res.err().unwrap()),
            "Confidential error: invalid nonce or public key"
        );
    }

    #[test]
    fn test_activated() {
        let peer_public_key = PublicKey::default();
        let public_key = PublicKey::default();
        let private_key = PrivateKey::default();
        let state_key = StateKey::default();
        let contract_key = KeyPair::new(public_key, private_key, state_key, vec![]);
        let nonce = Nonce::new([0; NONCE_SIZE]);
        let address = Address::default();
        assert_eq!(
            ConfidentialCtx {
                peer_public_key: Some(peer_public_key),
                contract: Some((address, contract_key)),
                next_nonce: Some(nonce),
                prev_block_hash: H256::default(),
                next_storage_nonce: None,
                // No storage encryption, so don't need a Deoxys-II instance.
                d2: None,
                key_manager: Arc::new(oasis_core_keymanager_client::mock::MockClient::new()),
                io_ctx: Context::background().freeze(),
                activated: true,
            }
            .activated(),
            true
        );
        assert_eq!(
            ConfidentialCtx {
                peer_public_key: None,
                contract: None,
                next_nonce: None,
                prev_block_hash: H256::default(),
                next_storage_nonce: None,
                // No storage encryption, so don't need a Deoxys-II instance.
                d2: None,
                key_manager: Arc::new(oasis_core_keymanager_client::mock::MockClient::new()),
                io_ctx: Context::background().freeze(),
                activated: false,
            }
            .activated(),
            false
        );
    }

    #[test]
    fn test_decrypt_tx_data_after_deactivate() {
        let peer_public_key = PublicKey::default();
        let public_key = PublicKey::default();
        let private_key = PrivateKey::default();
        let state_key = StateKey::default();
        let contract_key = KeyPair::new(public_key, private_key, state_key, vec![]);
        let nonce = Nonce::new([0; NONCE_SIZE]);
        let address = Address::default();
        let mut ctx = ConfidentialCtx {
            peer_public_key: Some(peer_public_key),
            contract: Some((address, contract_key)),
            next_nonce: Some(nonce),
            prev_block_hash: H256::default(),
            next_storage_nonce: None,
            // No storage encryption, so don't need a Deoxys-II instance.
            d2: None,
            key_manager: Arc::new(oasis_core_keymanager_client::mock::MockClient::new()),
            io_ctx: Context::background().freeze(),
            activated: false,
        };

        ctx.deactivate();
        let res = ctx.decrypt(Vec::new());

        assert_eq!(
            &format!("{}", res.err().unwrap()),
            "Confidential error: The confidential context must have a contract key when opening encrypted transaction data"
        );
    }
}
