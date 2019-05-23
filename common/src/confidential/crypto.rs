//! Encryption utilties for Web3(c).
//! Wraps the ekiden_core::mrae::sivaessha2 primitives with a set of encryption
//! methods that transparently encodes/decodes the Web3(c) wire format.

use ekiden_keymanager_client::{PrivateKey, PublicKey};
use ekiden_runtime::common::crypto::mrae::{
    deoxysii,
    nonce::{Nonce, NONCE_SIZE},
};
use failure::{format_err, Fallible, ResultExt};

/// Encrypts the given plaintext using the symmetric key derived from
/// peer_public_key and secret_key. Uses the given public_key to return
/// an encrypted payload of the form: nonce || public_key || cipher,
/// Allowing the receipient of the encrypted payload to decrypt with
/// the given nonce and public_key.
pub fn encrypt(
    plaintext: Vec<u8>,
    nonce: Nonce,
    peer_public_key: PublicKey,
    public_key: PublicKey,
    secret_key: PrivateKey,
) -> Fallible<Vec<u8>> {
    let ciphertext = deoxysii::box_seal(
        &nonce.clone(),
        plaintext.clone(),
        vec![],
        &peer_public_key.into(),
        &secret_key.into(),
    )?;
    Ok(encode_encryption(ciphertext, nonce, public_key))
}

/// Decrypts the given payload generated in the same manner by the encrypt method.
/// I.e., given an encrypted payload of the form nonce || public_key || cipher,
/// extracts the nonce and public key and uses them along with the given secret_key
/// the decrypt the cipher, returning the resulting Decryption struct.
pub fn decrypt(data: Option<Vec<u8>>, secret_key: PrivateKey) -> Fallible<Decryption> {
    if data.is_none() {
        return Ok(Decryption {
            plaintext: Default::default(),
            peer_public_key: Default::default(),
            nonce: Nonce::new([0; NONCE_SIZE]),
        });
    }
    let (nonce, peer_public_key, cipher) = split_encrypted_payload(data.unwrap())?;
    let plaintext = deoxysii::box_open(
        &nonce,
        cipher,
        vec![],
        &peer_public_key.into(),
        &secret_key.into(),
    )
    .with_context(|e| format!("payload open failed: {}", e))?;
    Ok(Decryption {
        plaintext,
        peer_public_key,
        nonce,
    })
}

/// The returned result of decrypting an encrypted payload, where
/// nonce and peer_public_key were used to encrypt the plaintext.
#[derive(Debug, Clone)]
pub struct Decryption {
    pub plaintext: Vec<u8>,
    pub nonce: Nonce,
    pub peer_public_key: PublicKey,
}

/// Packs the given paramaters into a Vec of the form nonce || public_key || ciphertext.
fn encode_encryption(mut ciphertext: Vec<u8>, nonce: Nonce, public_key: PublicKey) -> Vec<u8> {
    let mut encryption = nonce.to_vec();
    encryption.append(&mut public_key.as_ref().to_vec());
    encryption.append(&mut ciphertext);
    encryption
}

/// Assumes data is of the form  IV || PK || CIPHER.
/// Returns a tuple of each component.
fn split_encrypted_payload(data: Vec<u8>) -> Fallible<(Nonce, PublicKey, Vec<u8>)> {
    if data.len() < NONCE_SIZE + PublicKey::len() {
        return Err(format_err!("invalid nonce or public key"));
    }

    let mut nonce_inner: [u8; NONCE_SIZE] = Default::default();
    nonce_inner.copy_from_slice(&data[..NONCE_SIZE]);
    let nonce = Nonce::new(nonce_inner);

    let peer_public_key = PublicKey::from(&data[NONCE_SIZE..NONCE_SIZE + PublicKey::len()]);
    let cipher = data[NONCE_SIZE + PublicKey::len()..].to_vec();
    Ok((nonce, peer_public_key, cipher))
}
