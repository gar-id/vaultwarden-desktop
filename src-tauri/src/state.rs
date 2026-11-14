use std::collections::HashMap;
use std::sync::Mutex;
use rusqlite::Connection;
use rsa::RsaPrivateKey;
use zeroize::Zeroizing;

use vaultwarden_core::api::HttpClient;
use vaultwarden_core::crypto::SymmetricKey;
use vaultwarden_core::sync::VaultState;

#[derive(Clone)]
pub struct AccountSession {
    pub user_key: SymmetricKey,
    pub org_keys: HashMap<String, SymmetricKey>,
    pub rsa_private_key: Option<RsaPrivateKey>,
    #[allow(dead_code)]
    pub master_password_hash: Zeroizing<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub vault_state: VaultState,
    pub http_client: HttpClient,
}

pub struct AppState {
    /// 32-byte AES master key loaded from OS keyring on startup for accounts.enc
    pub master_key: [u8; 32],
    /// Active account ID
    pub active_account_id: Option<String>,
    /// Active SQLite connection for the current account
    pub db: Option<Connection>,
    /// Unlocked user key (cleared on lock)
    pub user_key: Option<SymmetricKey>,
    /// Unlocked organization keys (cleared on lock)
    pub org_keys: HashMap<String, SymmetricKey>,
    /// Unlocked user RSA private key (cleared on lock)
    pub rsa_private_key: Option<RsaPrivateKey>,
    /// Master password kept in memory while unlocked for re-auth
    pub master_password: Zeroizing<String>,
    /// Current vault state
    pub vault_state: VaultState,
    /// Authenticated HTTP client for the active account
    pub http_client: Option<HttpClient>,
    /// Active access token
    pub access_token: Option<String>,
    /// Active refresh token
    pub refresh_token: Option<String>,
    /// In-memory unlocked sessions keyed by account ID. Retained while the
    /// app is running so switching between accounts does not re-prompt for
    /// passwords until the application is closed or manually locked.
    pub sessions: HashMap<String, AccountSession>,
}

impl AppState {
    pub fn new(master_key: [u8; 32]) -> Self {
        Self {
            master_key,
            active_account_id: None,
            db: None,
            user_key: None,
            org_keys: HashMap::new(),
            rsa_private_key: None,
            master_password: Zeroizing::new(String::new()),
            vault_state: VaultState::Locked,
            http_client: None,
            access_token: None,
            refresh_token: None,
            sessions: HashMap::new(),
        }
    }
}

pub type SafeState = Mutex<AppState>;
