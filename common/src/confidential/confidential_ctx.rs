#![deny(warnings)]
use super::{crypto, key_manager::KeyManagerClient};
use ekiden_core::mrae::{
    nonce::{Nonce, NONCE_SIZE},
    sivaessha2::{SivAesSha2, KEY_SIZE},
};
use ekiden_keymanager_common::{ContractKey, PublicKeyType};
use ethcore::state::ConfidentialCtx as EthConfidentialCtx;
use ethereum_types::Address;

/// Facade for the underlying confidential contract services to be injected into
/// the parity state. Manages the confidential state--i.e., encryption keys and
/// nonce to use--to be encrypting under for a *single* transaction. Each
/// transaction for a confidential contract should have it's own ConfidentialCtx
/// that is closed at the end of the transaction's execution.
#[derive(Clone)]
pub struct ConfidentialCtx {
    /// The peer public key used for encryption. This should not change for an
    /// open confidential context. This is implicitly set by the `open` method.
    pub peer_public_key: Option<PublicKeyType>,
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
}

impl ConfidentialCtx {
    pub fn new() -> Self {
        Self {
            peer_public_key: None,
            contract_key: None,
            next_nonce: None,
        }
    }

    pub fn open_tx_data(&mut self, encrypted_tx_data: Vec<u8>) -> Result<Vec<u8>, String> {
        if self.contract_key.is_none() {
            return Err("The confidential context must have a contract key when opening encrypted transaction data".to_string());
        }
        let contract_secret_key = self.contract_key.as_ref().unwrap().input_keypair.get_sk();

        let decryption =
            crypto::decrypt(Some(encrypted_tx_data), &contract_secret_key).map_err(|err| {
                format!(
                    "Unable to decrypt transaction data: {}",
                    err.description().to_string()
                )
            })?;
        self.peer_public_key = Some(decryption.peer_public_key);

        let mut nonce = decryption.nonce;
        nonce
            .increment()
            .map_err(|err| err.description().to_string())?;
        self.next_nonce = Some(nonce);

        Ok(decryption.plaintext)
    }

    pub fn decrypt(&self, encrypted_tx_data: Vec<u8>) -> Result<Vec<u8>, String> {
        let contract_secret_key = self.contract_key.as_ref().unwrap().input_keypair.get_sk();
        let decryption = crypto::decrypt(Some(encrypted_tx_data), &contract_secret_key)
            .map_err(|err| err.description().to_string())?;

        Ok(decryption.plaintext)
    }
}

impl EthConfidentialCtx for ConfidentialCtx {
    fn open(
        &mut self,
        contract: Address,
        encrypted_tx_data: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, String> {
        if self.is_open() {
            return Err("Can't open a confidential context that's already open".to_string());
        }

        let contract_key = KeyManagerClient::contract_key(contract)?;
        if contract_key.is_none() {
            return Err("Could not fetch contract key".to_string());
        }

        self.contract_key = contract_key;

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

    fn encrypt(&mut self, data: Vec<u8>) -> Result<Vec<u8>, String> {
        if self.peer_public_key.is_none()
            || self.contract_key.is_none()
            || self.next_nonce.is_none()
        {
            return Err("must have key pair of a contract and peer and a next nonce".to_string());
        }

        let contract_key = &self.contract_key.as_ref().unwrap();
        let contract_pk = contract_key.input_keypair.get_pk();
        let contract_sk = contract_key.input_keypair.get_sk();

        let encrypted_payload = crypto::encrypt(
            data,
            self.next_nonce.clone().unwrap(),
            self.peer_public_key.clone().unwrap(),
            &contract_pk,
            &contract_sk,
        )
        .map_err(|err| err.description().to_string());

        self.next_nonce
            .as_mut()
            .unwrap()
            .increment()
            .map_err(|err| err.description().to_string())?;

        encrypted_payload
    }

    fn encrypt_storage(&self, data: Vec<u8>) -> Result<Vec<u8>, String> {
        let contract_key = &self
            .contract_key
            .as_ref()
            .expect("Should always have a contract key to encrypt storage");
        let state_key = contract_key.state_key;

        let key: Vec<u8> = state_key.as_ref()[..KEY_SIZE].to_vec();
        let nonce: Vec<u8> = state_key.as_ref()[KEY_SIZE..KEY_SIZE + NONCE_SIZE].to_vec();

        let mrae = SivAesSha2::new(key).unwrap();

        Ok(mrae.seal(nonce, data, vec![]).unwrap())
    }

    fn decrypt_storage(&self, data: Vec<u8>) -> Result<Vec<u8>, String> {
        let contract_key = &self
            .contract_key
            .as_ref()
            .expect("Should always have a contract key to decrypt storage");
        let state_key = contract_key.state_key;

        let key: Vec<u8> = state_key.as_ref()[..KEY_SIZE].to_vec();
        let nonce: Vec<u8> = state_key.as_ref()[KEY_SIZE..KEY_SIZE + NONCE_SIZE].to_vec();

        let mrae = SivAesSha2::new(key).unwrap();

        Ok(mrae.open(nonce, data, vec![]).unwrap())
    }

    fn create_long_term_public_key(&self, contract: Address) -> Result<(Vec<u8>, Vec<u8>), String> {
        KeyManagerClient::create_long_term_public_key(contract)
    }

    fn peer(&self) -> Option<Vec<u8>> {
        self.peer_public_key.as_ref().map(|pk| pk.to_vec())
    }
}
