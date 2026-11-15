use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

use super::error::CryptoError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum KdfType {
    Pbkdf2 = 0,
    Argon2id = 1,
}

impl Default for KdfType {
    fn default() -> Self {
        Self::Pbkdf2
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    pub kdf_type: KdfType,
    pub iterations: u32,
    pub memory: Option<u32>,      // In Megabytes (e.g. 64)
    pub parallelism: Option<u32>, // Number of threads/lanes (e.g. 4)
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            kdf_type: KdfType::Pbkdf2,
            iterations: 600_000,
            memory: None,
            parallelism: None,
        }
    }
}

impl KdfParams {
    pub fn pbkdf2(iterations: u32) -> Self {
        Self {
            kdf_type: KdfType::Pbkdf2,
            iterations,
            memory: None,
            parallelism: None,
        }
    }

    pub fn argon2id(iterations: u32, memory_mb: u32, parallelism: u32) -> Self {
        Self {
            kdf_type: KdfType::Argon2id,
            iterations,
            memory: Some(memory_mb),
            parallelism: Some(parallelism),
        }
    }
}

/// Derives the 256-bit (32-byte) Master Key from master password and email.
pub fn derive_master_key(
    password: &str,
    email: &str,
    params: &KdfParams,
) -> Result<[u8; 32], CryptoError> {
    let normalized_email = email.trim().to_lowercase();
    let mut master_key = [0u8; 32];

    log::info!(
        "[CRYPTO] derive_master_key: email='{}', pwd_len={}, kdf={:?}, iter={}, mem={:?}, par={:?}",
        normalized_email,
        password.len(),
        params.kdf_type,
        params.iterations,
        params.memory,
        params.parallelism
    );
    eprintln!(
        "[CRYPTO] derive_master_key: email='{}', pwd_len={}, kdf={:?}, iter={}, mem={:?}, par={:?}",
        normalized_email,
        password.len(),
        params.kdf_type,
        params.iterations,
        params.memory,
        params.parallelism
    );

    match params.kdf_type {
        KdfType::Pbkdf2 => {
            let salt = normalized_email.as_bytes();
            pbkdf2::pbkdf2_hmac::<Sha256>(
                password.as_bytes(),
                salt,
                params.iterations,
                &mut master_key,
            );
        }
        KdfType::Argon2id => {
            // Salt is SHA256(email) for Argon2id in Bitwarden
            let salt = Sha256::digest(normalized_email.as_bytes());

            let memory_mb = params.memory.unwrap_or(64);
            let memory_kib = if memory_mb >= 1024 {
                memory_mb
            } else {
                memory_mb * 1024
            };
            let iterations = params.iterations.max(1);
            let parallelism = params.parallelism.unwrap_or(4).max(1);

            let argon_params = Params::new(memory_kib, iterations, parallelism, Some(32))
                .map_err(|e| CryptoError::Kdf(format!("Invalid Argon2 params: {e}")))?;

            let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
            argon2
                .hash_password_into(password.as_bytes(), &salt, &mut master_key)
                .map_err(|e| CryptoError::Kdf(format!("Argon2 derivation failed: {e}")))?;
        }
    }

    Ok(master_key)
}

/// Computes the Master Password Hash used in OAuth token authentication.
/// Form: Base64(PBKDF2_HMAC_SHA256(key = master_key, salt = master_password, iter = 1))
pub fn derive_master_password_hash(
    master_key: &[u8; 32],
    password: &str,
) -> Result<String, CryptoError> {
    let mut hash = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(
        master_key,
        password.as_bytes(),
        1,
        &mut hash,
    );
    let encoded = BASE64.encode(hash);
    hash.zeroize();
    let preview = if encoded.len() > 10 {
        format!("{}...{}", &encoded[..4], &encoded[encoded.len() - 4..])
    } else {
        "***".to_string()
    };
    log::info!("[CRYPTO] derive_master_password_hash: generated hash (preview: {})", preview);
    eprintln!("[CRYPTO] derive_master_password_hash: generated hash (preview: {})", preview);
    Ok(encoded)
}

/// Expands the 256-bit Master Key into a 512-bit Stretched Master Key:
/// [0..32]: enc_key (derived with info b"enc")
/// [32..64]: mac_key (derived with info b"mac")
pub fn stretch_master_key(master_key: &[u8; 32]) -> Result<[u8; 64], CryptoError> {
    let hkdf = Hkdf::<Sha256>::from_prk(master_key)
        .map_err(|e| CryptoError::Kdf(format!("HKDF PRK init failed: {e}")))?;

    let mut enc_key = [0u8; 32];
    let mut mac_key = [0u8; 32];

    hkdf.expand(b"enc", &mut enc_key)
        .map_err(|e| CryptoError::Kdf(format!("HKDF expand enc failed: {e}")))?;

    hkdf.expand(b"mac", &mut mac_key)
        .map_err(|e| CryptoError::Kdf(format!("HKDF expand mac failed: {e}")))?;

    let mut stretched = [0u8; 64];
    stretched[0..32].copy_from_slice(&enc_key);
    stretched[32..64].copy_from_slice(&mac_key);

    enc_key.zeroize();
    mac_key.zeroize();

    Ok(stretched)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pbkdf2_key_derivation() {
        let params = KdfParams::pbkdf2(10_000);
        let key = derive_master_key("my-secret-password", "user@example.com", &params).unwrap();
        assert_ne!(key, [0u8; 32]);

        // Email normalization check: uppercase email should yield exact same key
        let key2 = derive_master_key("my-secret-password", " USER@EXAMPLE.COM ", &params).unwrap();
        assert_eq!(key, key2);

        // Different password yields different key
        let key3 = derive_master_key("other-password", "user@example.com", &params).unwrap();
        assert_ne!(key, key3);
    }

    #[test]
    fn test_argon2id_key_derivation() {
        let params = KdfParams::argon2id(2, 16, 2);
        let key = derive_master_key("my-secret-password", "user@example.com", &params).unwrap();
        assert_ne!(key, [0u8; 32]);

        let key2 = derive_master_key("my-secret-password", "USER@EXAMPLE.COM", &params).unwrap();
        assert_eq!(key, key2);
    }

    #[test]
    fn test_master_password_hash() {
        let params = KdfParams::pbkdf2(1000);
        let key = derive_master_key("password123", "test@domain.com", &params).unwrap();
        let hash = derive_master_password_hash(&key, "password123").unwrap();
        assert!(!hash.is_empty());
        // Base64 decoded length should be 32 bytes
        let decoded = BASE64.decode(&hash).unwrap();
        assert_eq!(decoded.len(), 32);
    }

    #[test]
    fn test_stretch_master_key() {
        let key = [42u8; 32];
        let stretched = stretch_master_key(&key).unwrap();
        assert_eq!(stretched.len(), 64);
        assert_ne!(&stretched[0..32], &stretched[32..64]);
    }
}
