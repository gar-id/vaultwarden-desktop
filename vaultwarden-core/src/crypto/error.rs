use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("KDF error: {0}")]
    Kdf(String),
    #[error("EncString format error: {0}")]
    InvalidEncString(String),
    #[error("HMAC verification failed — data tampered or invalid key")]
    HmacMismatch,
    #[error("Decryption error: {0}")]
    Decryption(String),
    #[error("Encryption error: {0}")]
    Encryption(String),
    #[error("RSA error: {0}")]
    Rsa(String),
    #[error("Base64 error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("TOTP error: {0}")]
    Totp(String),
}
