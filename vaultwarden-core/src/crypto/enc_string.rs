use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use cbc::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;

use super::error::CryptoError;

type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum EncryptionType {
    AesCbc256_B64 = 0,
    AesCbc256_HmacSha256_B64 = 2,
    Rsa2048_OaepSha1_B64 = 4,
}

/// Parsed components of an EncString.
pub struct ParsedEncString {
    pub enc_type: EncryptionType,
    pub iv: Option<Vec<u8>>,
    pub ct: Vec<u8>,
    pub mac: Option<Vec<u8>>,
}

/// Parses a Bitwarden EncString (e.g. "2.iv|ciphertext|mac" or "0.iv|ciphertext").
pub fn parse_enc_string(enc_str: &str) -> Result<ParsedEncString, CryptoError> {
    let parts: Vec<&str> = enc_str.splitn(2, '.').collect();
    if parts.len() != 2 {
        return Err(CryptoError::InvalidEncString(
            "EncString must contain a dot separator (e.g. '2.<data>')".into(),
        ));
    }

    let enc_type_num: u32 = parts[0]
        .parse()
        .map_err(|_| CryptoError::InvalidEncString(format!("Invalid encryption type prefix '{}'", parts[0])))?;

    let payload = parts[1];
    let payload_parts: Vec<&str> = payload.split('|').collect();

    match enc_type_num {
        0 => {
            if payload_parts.len() != 2 {
                return Err(CryptoError::InvalidEncString(
                    "Type 0 EncString requires 2 parts: iv|ciphertext".into(),
                ));
            }
            let iv = BASE64.decode(payload_parts[0])?;
            let ct = BASE64.decode(payload_parts[1])?;
            Ok(ParsedEncString {
                enc_type: EncryptionType::AesCbc256_B64,
                iv: Some(iv),
                ct,
                mac: None,
            })
        }
        2 => {
            if payload_parts.len() != 3 {
                return Err(CryptoError::InvalidEncString(
                    "Type 2 EncString requires 3 parts: iv|ciphertext|mac".into(),
                ));
            }
            let iv = BASE64.decode(payload_parts[0])?;
            let ct = BASE64.decode(payload_parts[1])?;
            let mac = BASE64.decode(payload_parts[2])?;
            Ok(ParsedEncString {
                enc_type: EncryptionType::AesCbc256_HmacSha256_B64,
                iv: Some(iv),
                ct,
                mac: Some(mac),
            })
        }
        4 => {
            if payload_parts.len() != 1 {
                return Err(CryptoError::InvalidEncString(
                    "Type 4 EncString requires 1 part: ciphertext".into(),
                ));
            }
            let ct = BASE64.decode(payload_parts[0])?;
            Ok(ParsedEncString {
                enc_type: EncryptionType::Rsa2048_OaepSha1_B64,
                iv: None,
                ct,
                mac: None,
            })
        }
        other => Err(CryptoError::InvalidEncString(format!(
            "Unsupported encryption type: {other}"
        ))),
    }
}

/// Decrypts an EncString into raw bytes using the provided symmetric key halves.
pub fn decrypt_to_bytes(
    enc_str: &str,
    enc_key: &[u8; 32],
    mac_key: &[u8; 32],
) -> Result<Vec<u8>, CryptoError> {
    let parsed = parse_enc_string(enc_str)?;

    match parsed.enc_type {
        EncryptionType::AesCbc256_HmacSha256_B64 => {
            let iv = parsed
                .iv
                .ok_or_else(|| CryptoError::InvalidEncString("Missing IV".into()))?;
            let mac_bytes = parsed
                .mac
                .ok_or_else(|| CryptoError::InvalidEncString("Missing MAC".into()))?;

            if iv.len() != 16 {
                return Err(CryptoError::InvalidEncString("IV must be 16 bytes".into()));
            }

            // Verify HMAC-SHA256 (encrypt-then-mac)
            let mut mac = HmacSha256::new_from_slice(mac_key)
                .map_err(|e| CryptoError::Decryption(e.to_string()))?;
            mac.update(&iv);
            mac.update(&parsed.ct);
            mac.verify_slice(&mac_bytes)
                .map_err(|_| CryptoError::HmacMismatch)?;

            // Decrypt AES-256-CBC
            let cipher = Aes256CbcDec::new(enc_key.into(), iv.as_slice().into());
            let plaintext = cipher
                .decrypt_padded_vec_mut::<Pkcs7>(&parsed.ct)
                .map_err(|e| CryptoError::Decryption(format!("AES-CBC decryption failed: {e:?}")))?;

            Ok(plaintext)
        }
        EncryptionType::AesCbc256_B64 => {
            let iv = parsed
                .iv
                .ok_or_else(|| CryptoError::InvalidEncString("Missing IV".into()))?;

            if iv.len() != 16 {
                return Err(CryptoError::InvalidEncString("IV must be 16 bytes".into()));
            }

            let cipher = Aes256CbcDec::new(enc_key.into(), iv.as_slice().into());
            let plaintext = cipher
                .decrypt_padded_vec_mut::<Pkcs7>(&parsed.ct)
                .map_err(|e| CryptoError::Decryption(format!("AES-CBC decryption failed: {e:?}")))?;

            Ok(plaintext)
        }
        EncryptionType::Rsa2048_OaepSha1_B64 => Err(CryptoError::Decryption(
            "RSA encrypted strings cannot be decrypted with symmetric keys; use decrypt_rsa instead".into(),
        )),
    }
}

/// Decrypts an EncString into a UTF-8 String.
pub fn decrypt_to_string(
    enc_str: &str,
    enc_key: &[u8; 32],
    mac_key: &[u8; 32],
) -> Result<String, CryptoError> {
    let bytes = decrypt_to_bytes(enc_str, enc_key, mac_key)?;
    String::from_utf8(bytes).map_err(CryptoError::Utf8)
}

/// Encrypts raw bytes into Bitwarden's default Type 2 EncString:
/// "2.<b64(iv)>|<b64(ct)>|<b64(mac)>"
pub fn encrypt_bytes(
    plaintext: &[u8],
    enc_key: &[u8; 32],
    mac_key: &[u8; 32],
) -> Result<String, CryptoError> {
    let mut iv = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut iv);

    let cipher = Aes256CbcEnc::new(enc_key.into(), (&iv).into());
    let ct = cipher.encrypt_padded_vec_mut::<Pkcs7>(plaintext);

    let mut mac = HmacSha256::new_from_slice(mac_key)
        .map_err(|e| CryptoError::Encryption(e.to_string()))?;
    mac.update(&iv);
    mac.update(&ct);
    let mac_result = mac.finalize().into_bytes();

    Ok(format!(
        "2.{}|{}|{}",
        BASE64.encode(iv),
        BASE64.encode(&ct),
        BASE64.encode(mac_result)
    ))
}

/// Encrypts a UTF-8 string into a Type 2 EncString.
pub fn encrypt_string(
    plaintext: &str,
    enc_key: &[u8; 32],
    mac_key: &[u8; 32],
) -> Result<String, CryptoError> {
    encrypt_bytes(plaintext.as_bytes(), enc_key, mac_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_2_encrypt_decrypt_roundtrip() {
        let enc_key = [1u8; 32];
        let mac_key = [2u8; 32];

        let original = "Hello, Vaultwarden & Bitwarden!";
        let enc = encrypt_string(original, &enc_key, &mac_key).unwrap();

        assert!(enc.starts_with("2."));

        let decrypted = decrypt_to_string(&enc, &enc_key, &mac_key).unwrap();
        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_hmac_tamper_detection() {
        let enc_key = [1u8; 32];
        let mac_key = [2u8; 32];

        let original = "Secret data";
        let enc = encrypt_string(original, &enc_key, &mac_key).unwrap();

        // Tamper with MAC key
        let wrong_mac_key = [3u8; 32];
        let err = decrypt_to_string(&enc, &enc_key, &wrong_mac_key).unwrap_err();
        match err {
            CryptoError::HmacMismatch => {}
            other => panic!("Expected HmacMismatch, got {other:?}"),
        }

        // Tamper with ciphertext in string
        let mut tampered = enc.clone();
        tampered.replace_range(10..12, "ZZ");
        let err2 = decrypt_to_string(&tampered, &enc_key, &mac_key).unwrap_err();
        assert!(matches!(err2, CryptoError::HmacMismatch | CryptoError::Base64(_)));
    }

    #[test]
    fn test_type_0_decrypt() {
        // Construct a manual type 0 string
        let enc_key = [5u8; 32];
        let iv = [9u8; 16];
        let cipher = Aes256CbcEnc::new((&enc_key).into(), (&iv).into());
        let ct = cipher.encrypt_padded_vec_mut::<Pkcs7>(b"legacy secret");

        let enc_str = format!("0.{}|{}", BASE64.encode(iv), BASE64.encode(&ct));
        let decrypted = decrypt_to_string(&enc_str, &enc_key, &[0u8; 32]).unwrap();
        assert_eq!(decrypted, "legacy secret");
    }
}
