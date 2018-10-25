use ekiden_keymanager_common::confidential;
use ekiden_core::mrae::sivaessha2::NONCE_SIZE;
use ethcore::state::{Encrypter as EthEncrypter, KeyManager as EthKeyManager};
use ethereum_types::Address;

/// Implementation of the parity KeyManager trait to inject into the ConfidentialVm.
pub struct KeyManager;
impl EthKeyManager for KeyManager {
    fn long_term_public_key(&self, contract: Address) -> Vec<u8> {
        let (pk, _sk) = confidential::default_contract_keys();
        pk.to_vec()
    }
}

/// Implementation of the Encrypter trait to inject into parity for confidential contracts.
pub struct Encrypter;
impl EthEncrypter for Encrypter {
    fn encrypt(
        &self,
        plaintext: Vec<u8>,
        peer_public_key: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        if peer_public_key.len() < 32 {
            return Err("public keys must be 32 bytes long".to_string());
        }
        // just generate arbitrary nonce for now (change this to a random nonce once we encrypt
        // with an actual key manager)
        let mut nonce = [1u8; NONCE_SIZE];
        let mut pub_key: [u8; 32] = Default::default();
        pub_key.copy_from_slice(&peer_public_key[..32]);
        confidential::encrypt(plaintext, nonce.to_vec(), pub_key)
            .map_err(|_| "could not decrypt".to_string())
    }

    /// Returns a tuple containing the nonce, public key, and plaintext
    /// used to generate the given cypher.
    fn decrypt(&self, cypher: Vec<u8>) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>), String> {
        let decryption = confidential::decrypt(Some(cypher)).map_err(|_| "Error".to_string())?;
        Ok((
            decryption.nonce,
            decryption.peer_public_key.to_vec(),
            decryption.plaintext,
        ))
    }
}
