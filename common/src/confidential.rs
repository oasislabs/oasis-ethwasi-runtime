use ekiden_core::mrae::sivaessha2::{SivAesSha2, KEY_SIZE, NONCE_SIZE};
use ekiden_keymanager_client::KeyManager as EkidenKeyManager;
use ekiden_keymanager_common::{confidential, ContractId, ContractKey, PublicKeyType};
use ethcore::state::ConfidentialCtx as EthConfidentialCtx;
use ethereum_types::Address;
use keccak_hash::keccak;

/// Facade for the underlying confidential contract services to be injected into
/// the parity state. Manages the current keys to be encrypting under.
pub struct ConfidentialCtx {
    /// The peer public key used for encryption. This should not change for an open
    /// confidential context. This is implicitly set by the `open` method.
    peer_public_key: Option<PublicKeyType>,
    /// The contract address and keys used for encryption. These keys may be swapped in
    /// an open confidential context, facilitating a confidential context switch to
    /// encrypt for the *same user* but under a different contract.
    contract_key: Option<ContractKey>,
}

impl ConfidentialCtx {
    pub fn new() -> Self {
        Self {
            peer_public_key: None,
            contract_key: None,
        }
    }

    pub fn open_tx_data(&mut self, encrypted_tx_data: Vec<u8>) -> Result<Vec<u8>, String> {
        // `open` must be called before this method.
        assert!(self.contract_key.is_some());

        let contract_secret_key = self.contract_key.as_ref().unwrap().input_keypair.get_sk();

        let decryption = confidential::decrypt(Some(encrypted_tx_data), &contract_secret_key)
            .map_err(|err| err.description().to_string())?;
        self.peer_public_key = Some(decryption.peer_public_key);

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

        self.contract_key = Some(KeyManager::contract_key(contract)?);

        let tx_data = if encrypted_tx_data.is_some() {
            self.open_tx_data(encrypted_tx_data.unwrap())?
        } else {
            vec![]
        };

        Ok(tx_data)
    }

    fn is_open(&self) -> bool {
        self.contract_key.is_some()
    }

    fn close(&mut self) {
        self.peer_public_key = None;
        self.contract_key = None;
    }

    fn encrypt(&self, data: Vec<u8>) -> Result<Vec<u8>, String> {
        if self.peer_public_key.is_none() || self.contract_key.is_none() {
            return Err("must have key pair of a contract and peer".to_string());
        }

        let contract_key = &self.contract_key.as_ref().unwrap();
        let contract_pk = contract_key.input_keypair.get_pk();
        let contract_sk = contract_key.input_keypair.get_sk();

        confidential::encrypt(
            data,
            random_nonce(),
            self.peer_public_key.clone().unwrap(),
            &contract_pk,
            &contract_sk,
        ).map_err(|err| err.description().to_string())
    }

    fn encrypt_storage(&self, data: Vec<u8>) -> Result<Vec<u8>, String> {
        let contract_key = &self.contract_key
            .as_ref()
            .expect("Should always have a contract key to encrypt storage");
        let state_key = contract_key.state_key;

        let key: Vec<u8> = state_key.as_ref()[..KEY_SIZE].to_vec();
        let nonce: Vec<u8> = state_key.as_ref()[KEY_SIZE..KEY_SIZE + NONCE_SIZE].to_vec();

        let mrae = SivAesSha2::new(key).unwrap();

        Ok(mrae.seal(nonce, data, vec![]).unwrap())
    }

    fn decrypt_storage(&self, data: Vec<u8>) -> Result<Vec<u8>, String> {
        let contract_key = &self.contract_key
            .as_ref()
            .expect("Should always have a contract key to decrypt storage");
        let state_key = contract_key.state_key;

        let key: Vec<u8> = state_key.as_ref()[..KEY_SIZE].to_vec();
        let nonce: Vec<u8> = state_key.as_ref()[KEY_SIZE..KEY_SIZE + NONCE_SIZE].to_vec();

        let mrae = SivAesSha2::new(key).unwrap();

        Ok(mrae.open(nonce, data, vec![]).unwrap())
    }

    fn create_long_term_public_key(&self, contract: Address) -> Result<Vec<u8>, String> {
        KeyManager::create_long_term_public_key(contract)
    }

    fn peer(&self) -> Option<Vec<u8>> {
        self.peer_public_key.as_ref().map(|pk| pk.to_vec())
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
    fn create_long_term_public_key(contract: Address) -> Result<Vec<u8>, String> {
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

    fn contract_key(address: Address) -> Result<ContractKey, String> {
        let contract_id = KeyManager::contract_id(address);
        let mut km = EkidenKeyManager::instance().expect("Should always have a key manager client");

        let (secret_key, state_key) = km.get_or_create_secret_keys(contract_id)
            .map_err(|err| err.description().to_string())?;
        let public_key = km.get_public_key(contract_id)
            .map_err(|err| err.description().to_string())?;

        Ok(ContractKey::new(public_key, secret_key, state_key))
    }
}

fn random_nonce() -> Vec<u8> {
    let mut nonce = [0u8; NONCE_SIZE];
    nonce.to_vec()
}
