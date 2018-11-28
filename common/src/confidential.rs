use ekiden_core::mrae::sivaessha2::NONCE_SIZE;
use ekiden_keymanager_client::KeyManager as EkidenKeyManager;
use ekiden_keymanager_common::{confidential, ContractId, PrivateKeyType, PublicKeyType};
use ethcore::state::ConfidentialCtx as EthConfidentialCtx;
use ethereum_types::Address;
use keccak_hash::keccak;

#[cfg(not(target_env = "sgx"))]
use rand::{OsRng as TheRng, Rng};
#[cfg(target_env = "sgx")]
use sgx_rand::{Rng, SgxRng as TheRng};

/// Facade for the underlying confidential contract services to be injected into
/// the parity state. Manages the current keys to be encrypting under.
pub struct ConfidentialCtx {
    /// The peer public key used for encryption. This should not change for an open
    /// confidential context. This is implicitly set by the `open` method.
    peer_public_key: Option<PublicKeyType>,
    /// The contract key pair used for encryption. These keys may be swapped in
    /// an open confidential context, facilitating a confidential context switch
    /// to encrypt for the *same user* under a different contract.
    contract_keypair: Option<(PublicKeyType, PrivateKeyType)>,
}

impl ConfidentialCtx {
    pub fn new() -> Self {
        Self {
            peer_public_key: None,
            contract_keypair: None,
        }
    }
}

impl EthConfidentialCtx for ConfidentialCtx {
    fn open(&mut self, encrypted_data: Vec<u8>, contract: Address) -> Result<Vec<u8>, String> {
        let (contract_pk, contract_sk) = KeyManager::contract_keypair(contract)?;

        let decryption = confidential::decrypt(Some(encrypted_data), &contract_sk)
            .map_err(|err| err.description().to_string())?;

        self.contract_keypair = Some((contract_pk, contract_sk));
        self.peer_public_key = Some(decryption.peer_public_key);

        Ok(decryption.plaintext)
    }

    fn is_open(&self) -> bool {
        self.peer_public_key.is_some() && self.contract_keypair.is_some()
    }

    fn close(&mut self) {
        self.peer_public_key = None;
        self.contract_keypair = None;
    }

    fn encrypt(&self, data: Vec<u8>) -> Result<Vec<u8>, String> {
        if self.peer_public_key.is_none() || self.contract_keypair.is_none() {
            return Err("must have key pair of a contract and peer".to_string());
        }

        let (contract_pk, contract_sk) = self.contract_keypair.clone().unwrap();

        confidential::encrypt(
            data,
            random_nonce(),
            self.peer_public_key.clone().unwrap(),
            &contract_pk,
            &contract_sk,
        ).map_err(|err| err.description().to_string())
    }

    fn create_long_term_pk(&self, contract: Address) -> Result<Vec<u8>, String> {
        KeyManager::create_long_term_pk(contract)
    }
}

/// Wrapper around the Ekiden key manager client to provide a more convenient
/// Ethereum address based interface along with runtime-specific utility methods.
pub struct KeyManager;
impl KeyManager {
    /// Returns the contract id for the given contract address. The contract_id
    /// is used to fetch keys for a contract.
    fn contract_id(contract: Address) -> ContractId {
        ContractId::from(&keccak(contract.to_vec())[..])
    }

    /// Creates and returns the long term public key for the given contract.
    /// If the key already exists, returns the existing key.
    fn create_long_term_pk(contract: Address) -> Result<Vec<u8>, String> {
        // if we're not in sgx, then don't try to access or create secret keys
        // this happens when running virtual confidential transactions
        // from the gateway via estimateGas
        if cfg!(not(target_env = "sgx")) {
            return Ok(vec![]);
        }

        let contract_id = Self::contract_id(contract);
        let mut km = EkidenKeyManager::instance().expect("Should always have a key manager client");

        // first create the keys
        km.get_or_create_secret_keys(contract_id);
        // then extract the long term key
        km.get_public_key(contract_id)
            .map_err(|err| err.description().to_string())
            .map(|key| key.to_vec())
    }

    fn contract_keypair(address: Address) -> Result<(PublicKeyType, PrivateKeyType), String> {
        let contract_id = KeyManager::contract_id(address);
        let mut km = EkidenKeyManager::instance().expect("Should always have a key manager client");

        let (secret_key, _state_key) = km.get_or_create_secret_keys(contract_id)
            .map_err(|err| err.description().to_string())?;
        let public_key = km.get_public_key(contract_id)
            .map_err(|err| err.description().to_string())?;

        Ok((public_key, secret_key))
    }
}

fn random_nonce() -> Vec<u8> {
    let mut nonce = [0u8; NONCE_SIZE];
    let mut rng = TheRng::new().unwrap();
    rng.fill_bytes(&mut nonce);
    nonce.to_vec()
}
