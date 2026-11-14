use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Keyring error: {0}")]
    Keyring(String),
    #[error("AES-GCM encryption error")]
    Encryption,
    #[error("AES-GCM decryption error — data tampered or wrong key")]
    Decryption,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Hex decode error: {0}")]
    Hex(#[from] hex::FromHexError),
    #[error("Profile not found: {0}")]
    NotFound(String),
}
