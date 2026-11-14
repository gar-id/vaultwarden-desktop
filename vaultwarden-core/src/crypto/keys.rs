use rand::RngCore;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs8::DecodePrivateKey;
use rsa::{Oaep, RsaPrivateKey};
use sha1::Sha1;
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::enc_string::{decrypt_to_bytes, decrypt_to_string, encrypt_bytes, encrypt_string, parse_enc_string, EncryptionType};
use super::error::CryptoError;

/// A 512-bit Bitwarden Symmetric Key:
/// - 256 bits (32 bytes) for AES-256 encryption
/// - 256 bits (32 bytes) for HMAC-SHA256 authentication
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SymmetricKey {
    pub enc_key: [u8; 32],
    pub mac_key: [u8; 32],
}

impl SymmetricKey {
    pub fn new(enc_key: [u8; 32], mac_key: [u8; 32]) -> Self {
        Self { enc_key, mac_key }
    }

    pub fn from_slice(slice: &[u8]) -> Result<Self, CryptoError> {
        if slice.len() != 64 {
            return Err(CryptoError::Decryption(format!(
                "Expected 64 bytes for symmetric key, got {}",
                slice.len()
            )));
        }
        let mut enc_key = [0u8; 32];
        let mut mac_key = [0u8; 32];
        enc_key.copy_from_slice(&slice[0..32]);
        mac_key.copy_from_slice(&slice[32..64]);
        Ok(Self { enc_key, mac_key })
    }

    pub fn to_bytes(&self) -> [u8; 64] {
        let mut out = [0u8; 64];
        out[0..32].copy_from_slice(&self.enc_key);
        out[32..64].copy_from_slice(&self.mac_key);
        out
    }

    pub fn generate() -> Self {
        let mut enc_key = [0u8; 32];
        let mut mac_key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut enc_key);
        rand::thread_rng().fill_bytes(&mut mac_key);
        Self { enc_key, mac_key }
    }

    pub fn decrypt_string(&self, enc_str: &str) -> Result<String, CryptoError> {
        decrypt_to_string(enc_str, &self.enc_key, &self.mac_key)
    }

    pub fn decrypt_bytes(&self, enc_str: &str) -> Result<Vec<u8>, CryptoError> {
        decrypt_to_bytes(enc_str, &self.enc_key, &self.mac_key)
    }

    pub fn encrypt_string(&self, plaintext: &str) -> Result<String, CryptoError> {
        encrypt_string(plaintext, &self.enc_key, &self.mac_key)
    }

    pub fn encrypt_bytes(&self, plaintext: &[u8]) -> Result<String, CryptoError> {
        encrypt_bytes(plaintext, &self.enc_key, &self.mac_key)
    }
}

/// Unwraps the user's primary Symmetric Key from `profile.key` using the
/// 64-byte Stretched Master Key (HKDF output).
pub fn unwrap_user_key(
    profile_key_enc: &str,
    stretched_master_key: &[u8; 64],
) -> Result<SymmetricKey, CryptoError> {
    let mut enc_key = [0u8; 32];
    let mut mac_key = [0u8; 32];
    enc_key.copy_from_slice(&stretched_master_key[0..32]);
    mac_key.copy_from_slice(&stretched_master_key[32..64]);

    let key_bytes = decrypt_to_bytes(profile_key_enc, &enc_key, &mac_key)?;
    enc_key.zeroize();
    mac_key.zeroize();

    SymmetricKey::from_slice(&key_bytes)
}

/// Parses an RSA private key from either PKCS#8 or PKCS#1 DER/PEM bytes.
pub fn parse_rsa_private_key(key_bytes: &[u8]) -> Result<RsaPrivateKey, CryptoError> {
    // Try PKCS#8 DER
    if let Ok(k) = RsaPrivateKey::from_pkcs8_der(key_bytes) {
        return Ok(k);
    }
    // Try PKCS#1 DER
    if let Ok(k) = RsaPrivateKey::from_pkcs1_der(key_bytes) {
        return Ok(k);
    }
    // Try PEM text format
    if let Ok(pem_str) = std::str::from_utf8(key_bytes) {
        if let Ok(k) = RsaPrivateKey::from_pkcs8_pem(pem_str) {
            return Ok(k);
        }
        if let Ok(k) = RsaPrivateKey::from_pkcs1_pem(pem_str) {
            return Ok(k);
        }
    }

    Err(CryptoError::Rsa("Failed to parse RSA private key (neither PKCS#8 nor PKCS#1)".into()))
}

/// Unwraps an organization's Symmetric Key from a Type 4 EncString
/// using the user's RSA-2048 private key (RSA-OAEP with SHA-1).
pub fn unwrap_org_key(
    org_key_enc: &str,
    rsa_private_key: &RsaPrivateKey,
) -> Result<SymmetricKey, CryptoError> {
    let parsed = parse_enc_string(org_key_enc)?;
    if parsed.enc_type != EncryptionType::Rsa2048_OaepSha1_B64 {
        return Err(CryptoError::InvalidEncString(
            "Org key must be Type 4 (Rsa2048_OaepSha1_B64)".into(),
        ));
    }

    let padding = Oaep::new::<Sha1>();
    let decrypted_bytes = rsa_private_key
        .decrypt(padding, &parsed.ct)
        .map_err(|e| CryptoError::Rsa(format!("RSA-OAEP decryption failed: {e}")))?;

    SymmetricKey::from_slice(&decrypted_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::enc_string::encrypt_bytes;
    use base64::Engine;
    use rsa::RsaPublicKey;

    #[test]
    fn test_symmetric_key_lifecycle() {
        let key = SymmetricKey::generate();
        let enc = key.encrypt_string("my password note").unwrap();
        let dec = key.decrypt_string(&enc).unwrap();
        assert_eq!(dec, "my password note");
    }

    #[test]
    fn test_unwrap_user_key() {
        let master_stretched = [77u8; 64];
        let mut enc_key = [0u8; 32];
        let mut mac_key = [0u8; 32];
        enc_key.copy_from_slice(&master_stretched[0..32]);
        mac_key.copy_from_slice(&master_stretched[32..64]);

        let user_key = SymmetricKey::generate();
        let enc_profile_key = encrypt_bytes(&user_key.to_bytes(), &enc_key, &mac_key).unwrap();

        let unwrapped = unwrap_user_key(&enc_profile_key, &master_stretched).unwrap();
        assert_eq!(unwrapped.to_bytes(), user_key.to_bytes());
    }

    #[test]
    fn test_rsa_org_key_unwrap() {
        let mut rng = rand::thread_rng();
        let priv_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let pub_key = RsaPublicKey::from(&priv_key);

        let org_symm_key = SymmetricKey::generate();

        // Encrypt with RSA-OAEP SHA-1 (as server does)
        let padding = Oaep::new::<Sha1>();
        let ct = pub_key.encrypt(&mut rng, padding, &org_symm_key.to_bytes()).unwrap();
        let enc_str = format!("4.{}", base64::engine::general_purpose::STANDARD.encode(ct));

        let unwrapped = unwrap_org_key(&enc_str, &priv_key).unwrap();
        assert_eq!(unwrapped.to_bytes(), org_symm_key.to_bytes());
    }
}
