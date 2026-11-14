use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Authentication failed: {0}")]
    Auth(String),
    #[error("Two-factor authentication required")]
    TwoFactorRequired {
        providers: Vec<i32>,
        auth_token: Option<String>,
    },
    #[error("Session expired — re-authentication required")]
    SessionExpired,
    #[error("Server response malformed: {0}")]
    Parse(String),
    #[error("Crypto error: {0}")]
    Crypto(#[from] crate::crypto::CryptoError),
}
