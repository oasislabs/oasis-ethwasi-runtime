#[cfg(feature = "test")]
use ekiden_core::random;
use ekiden_keymanager_client::KeyManager as EkidenKeyManager;
use ekiden_keymanager_common::{ContractId, ContractKey};
#[cfg(feature = "test")]
use ekiden_keymanager_common::{
    PublicKeyType, EMPTY_PRIVATE_KEY, EMPTY_PUBLIC_KEY, EMPTY_STATE_KEY,
};
use ethereum_types::Address;
use keccak_hash::keccak;
#[cfg(feature = "test")]
use std::collections::HashMap;
#[cfg(feature = "test")]
use std::sync::{Mutex, MutexGuard};

/// KeyManagerClient is a wrapper for an underlying KeyManager so that we can
/// swap the backend depending upon if we're testing or not. This allows us to
/// execute confidential contracts without running a key manager node.
pub struct KeyManagerClient;
#[cfg(not(feature = "test"))]
impl KeyManagerClient {
    /// Returns the tuple (public_key, signature_{KeyManager}(public_key)).
    pub fn create_long_term_public_key(contract: Address) -> Result<(Vec<u8>, Vec<u8>), String> {
        KeyManager::create_long_term_public_key(contract)
    }
    pub fn contract_key(address: Address) -> Result<ContractKey, String> {
        KeyManager::contract_key(address)
    }
}
#[cfg(feature = "test")]
impl KeyManagerClient {
    /// Returns the tuple (public_key, signature_{KeyManager}(public_key)).
    pub fn create_long_term_public_key(contract: Address) -> Result<(Vec<u8>, Vec<u8>), String> {
        TEST_KEY_MANAGER
            .lock()
            .unwrap()
            .create_long_term_public_key(contract)
    }
    pub fn contract_key(address: Address) -> Result<ContractKey, String> {
        TEST_KEY_MANAGER.lock().unwrap().contract_key(address)
    }
}

#[cfg(not(feature = "test"))]
#[derive(Debug)]
/// Wrapper around the Ekiden key manager client to provide a more convenient
/// Ethereum address based interface along with runtime-specific utility methods.
struct KeyManager;

#[cfg(not(feature = "test"))]
impl KeyManager {
    /// Returns the contract id for the given contract address. The contract_id
    /// is used to fetch keys for a contract.
    fn contract_id(contract: Address) -> ContractId {
        ContractId::from(&keccak(contract.to_vec())[..])
    }

    /// Creates and returns the long term public key for the given contract.
    /// If the key already exists, returns the existing key.
    /// Returns the tuple (public_key, signature_{KeyManager}(public_key)).
    fn create_long_term_public_key(contract: Address) -> Result<(Vec<u8>, Vec<u8>), String> {
        let contract_id = Self::contract_id(contract);
        let mut km = EkidenKeyManager::instance().expect("Should always have a key manager client");

        // first create the keys
        km.get_or_create_secret_keys(contract_id)
            .map_err(|err| err.description().to_string())?;
        // then extract the long term key
        km.long_term_public_key(contract_id)
            .map_err(|err| err.description().to_string())
            .map(|pk_payload| {
                (
                    pk_payload.public_key.to_vec(),
                    pk_payload.signature.to_vec(),
                )
            })
    }

    fn contract_key(address: Address) -> Result<ContractKey, String> {
        let contract_id = Self::contract_id(address);
        let mut km = EkidenKeyManager::instance().expect("Should always have a key manager client");

        let (secret_key, state_key) = km
            .get_or_create_secret_keys(contract_id)
            .map_err(|err| err.description().to_string())?;
        let public_key_payload = km
            .get_public_key(contract_id)
            .map_err(|err| err.description().to_string())?;

        Ok(ContractKey::new(
            public_key_payload.public_key,
            secret_key,
            state_key,
        ))
    }
}

#[cfg(feature = "test")]
lazy_static! {
    static ref TEST_KEY_MANAGER: Mutex<TestKeyManager> = Mutex::new(TestKeyManager::new());
}

/// Mock KeyManager to be used for tests. Locally generates and stores keys instead of
/// reaching out to a key manager node.
#[cfg(feature = "test")]
pub struct TestKeyManager {
    keys: HashMap<Address, ContractKey>,
}

#[cfg(feature = "test")]
impl TestKeyManager {
    fn new() -> Self {
        TestKeyManager {
            keys: HashMap::new(),
        }
    }

    pub fn instance<'a>() -> MutexGuard<'a, TestKeyManager> {
        TEST_KEY_MANAGER.lock().unwrap()
    }

    pub fn get_public_key(contract: Address) -> PublicKeyType {
        TEST_KEY_MANAGER
            .lock()
            .unwrap()
            .keys
            .get(&contract)
            .unwrap()
            .input_keypair
            .get_pk()
    }

    /// Returns the tuple (public_key, signature_{KeyManager}(public_key)).
    pub fn create_long_term_public_key(
        &mut self,
        contract: Address,
    ) -> Result<(Vec<u8>, Vec<u8>), String> {
        let contract_key = self.contract_key(contract)?;
        let public_key = contract_key.input_keypair.get_pk();
        Ok((public_key.to_vec(), vec![]))
    }

    pub fn contract_key(&mut self, contract: Address) -> Result<ContractKey, String> {
        if self.keys.contains_key(&contract) {
            Ok(self.keys.get(&contract).unwrap().clone())
        } else {
            let contract_key = Self::create_random_key();
            self.keys.insert(contract, contract_key.clone());
            Ok(contract_key)
        }
    }

    pub fn create_random_key() -> ContractKey {
        let mut seed = [0; 32];
        let mut public_key = EMPTY_PUBLIC_KEY;
        let mut private_key = EMPTY_PRIVATE_KEY;
        let mut state_key = EMPTY_STATE_KEY;

        random::get_random_bytes(&mut seed).expect("Should always get random bytes for the seed");
        sodalite::box_keypair_seed(&mut public_key, &mut private_key, &seed);
        random::get_random_bytes(&mut state_key)
            .expect("Should always get random bytes for a state key");

        ContractKey::new(public_key, private_key, state_key)
    }
}
