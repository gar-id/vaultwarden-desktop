use tauri::State;
use serde::Serialize;
use vaultwarden_core::{
    api::{self, ServerEndpoints},
    crypto,
    storage::accounts,
};
use crate::state::SafeState;
use std::sync::atomic::{AtomicU64, Ordering};

pub static LAST_ACTIVITY: AtomicU64 = AtomicU64::new(0);
pub static AUTO_LOCK_SECS: AtomicU64 = AtomicU64::new(900);
pub static CLIPBOARD_CLEAR_SECS: AtomicU64 = AtomicU64::new(30);

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct AppSettings {
    pub auto_lock_secs: u64,
    pub clipboard_clear_secs: u64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_lock_secs: 900,
            clipboard_clear_secs: 30,
        }
    }
}

pub fn load_app_settings() -> AppSettings {
    let path = accounts::config_root().join("settings.json");
    if let Ok(data) = std::fs::read(&path) {
        if let Ok(s) = serde_json::from_slice::<AppSettings>(&data) {
            return s;
        }
    }
    AppSettings::default()
}

pub fn save_app_settings_to_disk(settings: &AppSettings) {
    let dir = accounts::config_root();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(data) = serde_json::to_vec_pretty(settings) {
        let _ = std::fs::write(dir.join("settings.json"), data);
    }
}

#[tauri::command]
pub fn get_app_settings() -> AppSettings {
    AppSettings {
        auto_lock_secs: AUTO_LOCK_SECS.load(Ordering::Relaxed),
        clipboard_clear_secs: CLIPBOARD_CLEAR_SECS.load(Ordering::Relaxed),
    }
}

#[tauri::command]
pub fn set_app_settings(settings: AppSettings) {
    AUTO_LOCK_SECS.store(settings.auto_lock_secs, Ordering::Relaxed);
    CLIPBOARD_CLEAR_SECS.store(settings.clipboard_clear_secs, Ordering::Relaxed);
    save_app_settings_to_disk(&settings);
}

#[tauri::command]
pub fn record_user_activity() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    LAST_ACTIVITY.store(now, Ordering::Relaxed);
}

#[derive(Serialize)]
pub struct UnlockStatus {
    pub success: bool,
    pub message: String,
    pub online: bool,
}

#[tauri::command]
pub async fn unlock_vault(passphrase: String, state: State<'_, SafeState>) -> Result<UnlockStatus, String> {
    let (account_id, server_url, email, protected_key, protected_priv_key, refresh_token, saved_kdf_params, client) = {
        let s = state.lock().unwrap();
        let account_id = s.active_account_id.clone()
            .ok_or("No active account — select an account first")?;

        let store = accounts::load(&s.master_key).map_err(|e| e.to_string())?;
        let account = store.accounts.iter().find(|a| a.id == account_id)
            .ok_or("Active account not found")?;

        let client = api::build_client();
        let saved_kdf = api::PreloginResponse {
            kdf: account.kdf_type,
            kdf_iterations: account.kdf_iterations,
            kdf_memory: account.kdf_memory,
            kdf_parallelism: account.kdf_parallelism,
            kdf_settings: None,
        };
        (
            account_id,
            account.server_url.clone(),
            account.email.clone(),
            account.protected_key.clone(),
            account.protected_private_key.clone(),
            account.refresh_token.clone(),
            saved_kdf,
            client,
        )
    };

    let endpoints = ServerEndpoints::from_base_url(&server_url);

    // 1. Prelogin to get KDF params (fallback to saved account KDF params)
    let prelogin = match api::prelogin(&client, &endpoints.identity_url, &email).await {
        Ok(res) => res,
        Err(_) => saved_kdf_params,
    };
    let kdf_params = prelogin.to_kdf_params();

    // 2. Derive Master Key & Stretched Master Key
    let master_key = crypto::derive_master_key(&passphrase, &email, &kdf_params)
        .map_err(|e| format!("Master key derivation failed: {e}"))?;

    let stretched = crypto::stretch_master_key(&master_key)
        .map_err(|e| format!("HKDF stretching failed: {e}"))?;

    // 3. Unwrap User Symmetric Key
    let protected_key_str = protected_key.ok_or("Account missing protected encryption key")?;
    let user_key = crypto::unwrap_user_key(&protected_key_str, &stretched)
        .map_err(|_| "Invalid master password".to_string())?;

    // 4. Unwrap User RSA Private Key if present
    let rsa_private_key = if let Some(ref priv_enc) = protected_priv_key {
        if let Ok(decrypted_bytes) = user_key.decrypt_bytes(priv_enc) {
            crypto::parse_rsa_private_key(&decrypted_bytes).ok()
        } else {
            None
        }
    } else {
        None
    };

    // 5. Re-authenticate / Refresh access token
    let mut online = false;
    let mut access_token = None;
    let mut new_refresh = refresh_token.clone();

    if let Some(ref rt) = refresh_token {
        if let Ok(tok) = api::refresh_access_token(&client, &endpoints.identity_url, rt).await {
            online = true;
            access_token = Some(tok.access_token);
            if tok.refresh_token.is_some() {
                new_refresh = tok.refresh_token;
            }
        }
    }

    if !online {
        // Try password grant login
        if let Ok(hash) = crypto::derive_master_password_hash(&master_key, &passphrase) {
            let dev_id = uuid::Uuid::new_v4().to_string();
            if let Ok(tok) = api::login(&client, &endpoints.identity_url, &email, &hash, &dev_id, None).await {
                online = true;
                access_token = Some(tok.access_token);
                new_refresh = tok.refresh_token;
            }
        }
    }

    // 6. Update AppState
    let mut s = state.lock().unwrap();
    if s.active_account_id.as_deref() != Some(account_id.as_str()) {
        return Err("Active account changed during unlock".into());
    }

    let vault_state = if online {
        vaultwarden_core::sync::VaultState::OnlineReady
    } else {
        vaultwarden_core::sync::VaultState::OfflineRead
    };

    s.user_key = Some(user_key.clone());
    s.rsa_private_key = rsa_private_key.clone();
    *s.master_password = passphrase.clone();
    s.http_client = Some(client.clone());
    s.access_token = access_token.clone();
    s.refresh_token = new_refresh.clone();
    s.vault_state = vault_state;

    // Persist freshly issued tokens — refresh tokens rotate, so keeping a stale
    // one on disk would break token refresh after the app restarts.
    if s.access_token.is_some() {
        if let Ok(mut store) = accounts::load(&s.master_key) {
            if let Some(account) = store.accounts.iter_mut().find(|a| a.id == account_id) {
                account.access_token = s.access_token.clone();
                account.refresh_token = s.refresh_token.clone();
                let _ = accounts::save(&s.master_key, &store);
            }
        }
    }

    // Unwrap Org keys from SQLite DB if organizations table has keys
    let mut unwrapped_org_keys = std::collections::HashMap::new();
    if let Some(ref conn) = s.db {
        if let Some(ref rsa_priv) = s.rsa_private_key {
            if let Ok(orgs) = vaultwarden_core::storage::db::list_organizations(conn) {
                for org in orgs {
                    if !org.key_enc.is_empty() {
                        if let Ok(unwrapped_org_key) = crypto::unwrap_org_key(&org.key_enc, rsa_priv) {
                            unwrapped_org_keys.insert(org.id, unwrapped_org_key);
                        }
                    }
                }
            }
        }
    }
    s.org_keys.extend(unwrapped_org_keys);

    let current_org_keys = s.org_keys.clone();
    s.sessions.insert(
        account_id.clone(),
        crate::state::AccountSession {
            user_key,
            org_keys: current_org_keys,
            rsa_private_key,
            master_password_hash: zeroize::Zeroizing::new(passphrase),
            access_token,
            refresh_token: new_refresh,
            vault_state,
            http_client: client,
        },
    );

    record_user_activity();

    Ok(UnlockStatus {
        success: true,
        message: if online {
            "Vault unlocked".into()
        } else {
            "Vault unlocked (offline — showing cached data only)".into()
        },
        online,
    })
}

#[tauri::command]
pub fn lock_vault(state: State<'_, SafeState>) -> Result<(), String> {
    let mut s = state.lock().unwrap();
    if let Some(active_id) = s.active_account_id.clone() {
        s.sessions.remove(&active_id);
    }
    s.user_key = None;
    s.org_keys.clear();
    s.rsa_private_key = None;
    *s.master_password = String::new();
    s.access_token = None;
    s.refresh_token = None;
    s.vault_state = vaultwarden_core::sync::VaultState::Locked;
    Ok(())
}

#[derive(Serialize)]
pub struct VaultStateDto {
    pub locked: bool,
    pub state: String,
}

#[tauri::command]
pub fn get_vault_state(state: State<'_, SafeState>) -> VaultStateDto {
    let s = state.lock().unwrap();
    let label = match s.vault_state {
        vaultwarden_core::sync::VaultState::Locked => "locked",
        vaultwarden_core::sync::VaultState::OnlineReady => "online",
        vaultwarden_core::sync::VaultState::OfflineRead => "offline",
        vaultwarden_core::sync::VaultState::Syncing => "syncing",
    };
    VaultStateDto {
        locked: s.vault_state == vaultwarden_core::sync::VaultState::Locked,
        state: label.into(),
    }
}
