//! Encryption utilties to wrap the ekiden mrae box, transparently
//! encoding/decoding the ciphertext layout:
//!
//! PUBLIC_KEY || CIPHER_LEN || AAD_LEN || CIPHER || AAD || NONCE.

use std::convert::TryInto;

use failure::{format_err, Fallible, ResultExt};
use oasis_core_keymanager_client::{PrivateKey, PublicKey};
use oasis_core_runtime::common::crypto::mrae::{
    deoxysii,
    nonce::{Nonce, NONCE_SIZE},
};

/// Number of bytes representing the CIPHER_LEN parameter of the confidential
/// wire format.
const CIPHER_LEN_SIZE: usize = 8;
/// Number of bytes representing the AAD_LEN paramater of the confidential wire
/// format.
const AAD_LEN_SIZE: usize = 8;

/// Encrypts the given plaintext using the symmetric key derived from
/// peer_public_key and secret_key. Uses the given public_key to return
/// an encrypted payload with the following layout:
///
/// PUBLIC_KEY || CIPHER_LEN || AAD_LEN || CIPHER || AAD || NONCE.
///
/// Allowing the receipient of the encrypted payload to decrypt with
/// the given nonce and public_key.
pub fn encrypt(
    plaintext: Vec<u8>,
    nonce: Nonce,
    peer_public_key: PublicKey,
    public_key: PublicKey,
    secret_key: PrivateKey,
    aad: Vec<u8>,
) -> Fallible<Vec<u8>> {
    let ciphertext = deoxysii::box_seal(
        &nonce.clone(),
        plaintext.clone(),
        aad.clone(),
        &peer_public_key.into(),
        &secret_key.into(),
    )?;
    Ok(encode_encryption(ciphertext, nonce, public_key, aad))
}

/// Decrypts the given payload generated in the same manner by the encrypt method.
/// extracts the nonce and public key and uses them along with the given secret_key
/// to decrypt the cipher, returning the resulting Decryption struct.
pub fn decrypt(data: Option<Vec<u8>>, secret_key: PrivateKey) -> Fallible<Decryption> {
    if data.is_none() {
        return Ok(Decryption {
            plaintext: Default::default(),
            peer_public_key: Default::default(),
            nonce: Nonce::new([0; NONCE_SIZE]),
            aad: Default::default(),
        });
    }
    let (peer_public_key, _, _, cipher, aad, nonce) = split_encrypted_payload(data.unwrap())?;
    let plaintext = deoxysii::box_open(
        &nonce,
        cipher,
        aad.clone(),
        &peer_public_key.into(),
        &secret_key.into(),
    )
    .with_context(|e| format!("payload open failed: {}", e))?;
    Ok(Decryption {
        plaintext,
        peer_public_key,
        nonce,
        aad,
    })
}

/// The returned result of decrypting an encrypted payload, where
/// nonce and peer_public_key were used to encrypt the plaintext.
#[derive(Debug, Clone)]
pub struct Decryption {
    pub plaintext: Vec<u8>,
    pub nonce: Nonce,
    pub peer_public_key: PublicKey,
    pub aad: Vec<u8>,
}

/// Packs the given paramaters into the encoded ciphertext layout.
fn encode_encryption(
    mut ciphertext: Vec<u8>,
    nonce: Nonce,
    public_key: PublicKey,
    mut aad: Vec<u8>,
) -> Vec<u8> {
    let mut encryption = public_key.as_ref().to_vec();
    encryption.append(&mut (ciphertext.len() as u64).to_le_bytes().to_vec());
    encryption.append(&mut (aad.len() as u64).to_le_bytes().to_vec());
    encryption.append(&mut ciphertext);
    encryption.append(&mut aad);
    encryption.append(&mut nonce.to_vec());

    encryption
}

/// Assumes data is of the form:
///
/// PUBLIC_KEY || CIPHER_LEN || AAD_LEN || CIPHER || AAD || NONCE.
///
/// Returns a tuple of each component.
fn split_encrypted_payload(
    data: Vec<u8>,
) -> Fallible<(PublicKey, u64, u64, Vec<u8>, Vec<u8>, Nonce)> {
    if data.len() < PublicKey::len() + NONCE_SIZE + CIPHER_LEN_SIZE + AAD_LEN_SIZE {
        return Err(format_err!("invalid nonce or public key"));
    }

    let peer_public_key = PublicKey::from(&data[..PublicKey::len()]);

    let cipher_len_start = PublicKey::len();
    let cipher_len_end = cipher_len_start + CIPHER_LEN_SIZE;
    let mut cipher_array = [0u8; 8];
    cipher_array.copy_from_slice(&data[cipher_len_start..cipher_len_end]);
    let cipher_len: usize = u64::from_le_bytes(cipher_array).try_into()?;

    let aad_len_start = cipher_len_end;
    let aad_len_end = aad_len_start + AAD_LEN_SIZE;
    let mut aad_array = [0u8; 8];
    aad_array.copy_from_slice(&data[aad_len_start..aad_len_end]);
    let aad_len: usize = u64::from_le_bytes(aad_array).try_into()?;

    let expected_data_length =
        PublicKey::len() + CIPHER_LEN_SIZE + AAD_LEN_SIZE + cipher_len + aad_len + NONCE_SIZE;
    if data.len() != expected_data_length {
        return Err(format_err!("invalid size for ciphertext"));
    }

    let cipher_start = aad_len_end;
    let cipher_end = cipher_start + cipher_len;
    let cipher = data[cipher_start..cipher_end].to_vec();

    let aad_start = cipher_end;
    let aad_end = aad_start + aad_len;
    let aad = data[aad_start..aad_end].to_vec();

    let nonce_start = aad_end;
    let nonce_end = nonce_start + NONCE_SIZE;
    let mut nonce_inner: [u8; NONCE_SIZE] = Default::default();
    nonce_inner.copy_from_slice(&data[nonce_start..nonce_end]);
    let nonce = Nonce::new(nonce_inner);

    Ok((
        peer_public_key,
        cipher_len as u64,
        aad_len as u64,
        cipher,
        aad,
        nonce,
    ))
}
