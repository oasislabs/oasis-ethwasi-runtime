//! Encryption utilties for Web3(c).
//! Wraps the ekiden_core::mrae::sivaessha2 primitives with a set of encryption
//! methods that transparently encodes/decodes the Web3(c) wire format.

use ekiden_keymanager_client::{PrivateKey, PublicKey};
use ekiden_runtime::common::crypto::mrae::{
    deoxysii,
    nonce::{Nonce, NONCE_SIZE},
};
use failure::{format_err, Fallible, ResultExt};

/// Number of bytes representing the CIPHER_LEN parameter of the confidential
/// wire format.
const CIPHER_LEN_SIZE: usize = 8;
/// Number of bytes representing the AAD_LEN paramater of the confidential wire
/// format.
const AAD_LEN_SIZE: usize = 8;

/// Encrypts the given plaintext using the symmetric key derived from
/// peer_public_key and secret_key. Uses the given public_key to return
/// an encrypted payload of the form:
///
/// public_key || cipher_len || aad_len || cipher || aad || nonce.
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
    println!("peer public key = {:?}", peer_public_key);
    println!("cipher = {:?}", cipher);
    println!("aad = {:?}", aad);
    println!("nonce = {:?}", nonce);
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

/// Packs the given paramaters into a Vec of the form nonce || public_key || ciphertext.
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

/// Assumes data is of the form  IV || PK || CIPHER.
/// Returns a tuple of each component.
fn split_encrypted_payload(
    data: Vec<u8>
) -> Fallible<(PublicKey, u64, u64, Vec<u8>, Vec<u8>, Nonce)> {
    println!("split payload***************************");
    println!("data = {:?}", data);
    if data.len() < PublicKey::len() + NONCE_SIZE + CIPHER_LEN_SIZE + AAD_LEN_SIZE {
        return Err(format_err!("invalid nonce or public key"));
    }

    let peer_public_key = PublicKey::from(&data[..PublicKey::len()]);

    let cipher_len_start = PublicKey::len();
    let cipher_len_end = cipher_len_start + CIPHER_LEN_SIZE;
    let mut cipher_array = [0u8; 8];
    cipher_array.copy_from_slice(&data[cipher_len_start..cipher_len_end]);
    println!("cipiher array  = {:?}", cipher_array.to_vec());
    let cipher_len = u64::from_le_bytes(cipher_array);
    println!("cipher len = {:?}", cipher_len);

    let aad_len_start = cipher_len_end;
    let aad_len_end = aad_len_start + AAD_LEN_SIZE;
    let mut aad_array = [0u8; 8];
    aad_array.copy_from_slice(&data[aad_len_start..aad_len_end]);
    let aad_len = u64::from_le_bytes(aad_array);
    println!("aad_len = {:?}", aad_len);

    let cipher_start = aad_len_end;
    let cipher_end = cipher_start + cipher_len as usize;
    let cipher = data[cipher_start..cipher_end].to_vec();

    println!("cipher = {:?}", cipher);

    let aad_start = cipher_end;
    let aad_end = aad_start + aad_len as usize;
    let aad = data[aad_start..aad_end].to_vec();

    println!("aad = {:?}", aad);

    let nonce_start = aad_end;
    let nonce_end = nonce_start + NONCE_SIZE;
    let mut nonce_inner: [u8; NONCE_SIZE] = Default::default();
    nonce_inner.copy_from_slice(&data[nonce_start..nonce_end]);
    let nonce = Nonce::new(nonce_inner);

    println!("nonce = {:?}", nonce);

    println!("*********************************");

    Ok((peer_public_key, cipher_len, aad_len, cipher, aad, nonce))
}
