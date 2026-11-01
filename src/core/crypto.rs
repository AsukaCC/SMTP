use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use anyhow::{Result, anyhow};
use base64::{Engine, engine::general_purpose::STANDARD};
use rand::{RngCore, rngs::OsRng};
use rsa::{RsaPrivateKey, RsaPublicKey, pkcs1::EncodeRsaPrivateKey, pkcs8::EncodePublicKey};
use sha2::{Digest, Sha256};

pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}
pub fn hash(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

pub fn encrypt(key: &[u8; 32], context: &str, plaintext: &str) -> Result<String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| anyhow!("Invalid encryption key"))?;
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let payload = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plaintext.as_bytes(),
                aad: context.as_bytes(),
            },
        )
        .map_err(|_| anyhow!("Encryption failed"))?;
    Ok(STANDARD.encode([nonce.as_slice(), &payload].concat()))
}
pub fn decrypt(key: &[u8; 32], context: &str, ciphertext: &str) -> Result<String> {
    let bytes = STANDARD.decode(ciphertext)?;
    if bytes.len() < 28 {
        return Err(anyhow!("Invalid encrypted key"));
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| anyhow!("Invalid encryption key"))?;
    let plain = cipher
        .decrypt(
            Nonce::from_slice(&bytes[..12]),
            Payload {
                msg: &bytes[12..],
                aad: context.as_bytes(),
            },
        )
        .map_err(|_| anyhow!("DKIM key cannot be decrypted"))?;
    Ok(String::from_utf8(plain)?)
}
pub fn generate_dkim() -> Result<(String, String)> {
    let key = RsaPrivateKey::new(&mut OsRng, 2048)?;
    let public = STANDARD.encode(RsaPublicKey::from(&key).to_public_key_der()?.as_bytes());
    let private = key.to_pkcs1_pem(Default::default())?.to_string();
    Ok((private, public))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encryption_is_random_and_bound_to_domain() {
        let key = [7; 32];
        let a = encrypt(&key, "a.test", "private").unwrap();
        assert_ne!(a, encrypt(&key, "a.test", "private").unwrap());
        assert_eq!(decrypt(&key, "a.test", &a).unwrap(), "private");
        assert!(decrypt(&key, "b.test", &a).is_err());
        assert!(decrypt(&[8; 32], "a.test", &a).is_err());
    }
}
