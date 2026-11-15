use tauri::State;
use serde::{Deserialize, Serialize};
use vaultwarden_core::{
    api::{self, ServerEndpoints},
    crypto,
    storage::accounts::{self, Account},
};
use crate::state::SafeState;

#[derive(Serialize)]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub server_url: String,
    pub user_email: String,
    pub last_sync_timestamp: i64,
}

#[derive(Deserialize)]
pub struct AddProfileInput {
    pub name: String,
    pub server_url: String,
    pub user_email: String,
    pub master_password: Option<String>,
    pub armored_user_private_key: Option<String>,
    pub two_factor_provider: Option<i32>,
    pub two_factor_token: Option<String>,
    pub two_factor_remember: Option<bool>,
    pub device_id: Option<String>,
}

#[derive(Serialize)]
pub struct ServerIdentityDto {
    pub fingerprint: String,
    pub public_key: String,
}

#[tauri::command]
pub async fn discover_server(_server_url: String) -> Result<ServerIdentityDto, String> {
    Ok(ServerIdentityDto {
        fingerprint: String::new(),
        public_key: String::new(),
    })
}

#[tauri::command]
pub fn list_profiles(state: State<'_, SafeState>) -> Result<Vec<ProfileSummary>, String> {
    let s = state.lock().unwrap();
    let store = accounts::load(&s.master_key).map_err(|e| e.to_string())?;
    Ok(store.accounts.iter().map(|a| ProfileSummary {
        id: a.id.clone(),
        name: a.name.clone(),
        server_url: a.server_url.clone(),
        user_email: a.email.clone(),
        last_sync_timestamp: a.last_sync_timestamp,
    }).collect())
}

#[tauri::command]
pub async fn add_profile(input: AddProfileInput, state: State<'_, SafeState>) -> Result<ProfileSummary, String> {
    let endpoints = ServerEndpoints::from_base_url(&input.server_url);
    let client = api::build_client();

    let email = input.user_email.trim().to_lowercase();
    let master_pass = input.master_password
        .or(input.armored_user_private_key)
        .ok_or("Master password is required")?;

    log::info!("[IPC] add_profile called: server='{}', email='{}', pwd_len={}", input.server_url, email, master_pass.len());
    eprintln!("[IPC] add_profile called: server='{}', email='{}', pwd_len={}", input.server_url, email, master_pass.len());
    log::info!("[IPC] endpoints: identity='{}', api='{}'", endpoints.identity_url, endpoints.api_url);
    eprintln!("[IPC] endpoints: identity='{}', api='{}'", endpoints.identity_url, endpoints.api_url);

    // 1. Fetch prelogin config (KDF type and iterations)
    let prelogin = api::prelogin(&client.client, &endpoints.identity_url, &email)
        .await
        .map_err(|e| format!("Prelogin failed: {e}"))?;

    let kdf_params = prelogin.to_kdf_params();
    log::info!("[IPC] KDF params for login: {:?}", kdf_params);
    eprintln!("[IPC] KDF params for login: {:?}", kdf_params);

    // 2. Derive Master Key and Master Password Hash
    let master_key = crypto::derive_master_key(&master_pass, &email, &kdf_params)
        .map_err(|e| format!("Master key derivation failed: {e}"))?;

    let master_password_hash = crypto::derive_master_password_hash(&master_key, &master_pass)
        .map_err(|e| format!("Password hash derivation failed: {e}"))?;

    let device_id = input.device_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let two_factor = input.two_factor_token.as_ref().map(|t| {
        api::TwoFactorInput {
            provider: input.two_factor_provider.unwrap_or(0),
            token: t.trim().to_string(),
            remember: input.two_factor_remember.unwrap_or(true),
        }
    });

    // 3. Authenticate with server
    let token_res = match api::login(
        &client.client,
        &endpoints.identity_url,
        &email,
        &master_password_hash,
        &device_id,
        two_factor,
    ).await {
        Ok(t) => t,
        Err(api::ApiError::TwoFactorRequired { providers, auth_token: _ }) => {
            // If email 2FA is the only available provider, trigger sending email
            if providers.contains(&1) && !providers.contains(&0) {
                let _ = api::send_two_factor_email(&client.client, &endpoints.api_url, &email, &master_password_hash).await;
            }
            let provs_str = providers.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",");
            return Err(format!("2FA_REQUIRED:{device_id}:{provs_str}"));
        }
        Err(e) => return Err(format!("Login failed: {e}")),
    };

    let account = Account {
        id: uuid::Uuid::new_v4().to_string(),
        name: input.name,
        server_url: input.server_url,
        email,
        access_token: Some(token_res.access_token),
        refresh_token: token_res.refresh_token,
        protected_key: token_res.key,
        protected_private_key: token_res.private_key,
        last_sync_timestamp: 0,
        auto_lock_minutes: 15,
        device_id,
        kdf_type: Some(match kdf_params.kdf_type {
            crypto::KdfType::Pbkdf2 => 0,
            crypto::KdfType::Argon2id => 1,
        }),
        kdf_iterations: Some(kdf_params.iterations),
        kdf_memory: kdf_params.memory,
        kdf_parallelism: kdf_params.parallelism,
    };

    let s = state.lock().unwrap();
    accounts::add_account(&s.master_key, account.clone()).map_err(|e| e.to_string())?;

    Ok(ProfileSummary {
        id: account.id,
        name: account.name,
        server_url: account.server_url,
        user_email: account.email,
        last_sync_timestamp: 0,
    })
}

#[tauri::command]
pub fn remove_profile(profile_id: String, state: State<'_, SafeState>) -> Result<(), String> {
    let mut s = state.lock().unwrap();
    accounts::remove_account(&s.master_key, &profile_id).map_err(|e| e.to_string())?;

    if s.active_account_id.as_deref() == Some(profile_id.as_str()) {
        s.active_account_id = None;
        s.db = None;
        s.user_key = None;
        s.org_keys.clear();
        s.rsa_private_key = None;
        *s.master_password = String::new();
        s.http_client = None;
        s.access_token = None;
        s.refresh_token = None;
        s.vault_state = vaultwarden_core::sync::VaultState::Locked;
    }
    s.sessions.remove(&profile_id);

    Ok(())
}

#[tauri::command]
pub fn switch_profile(profile_id: String, state: State<'_, SafeState>) -> Result<String, String> {
    let mut s = state.lock().unwrap();

    // Save current session if unlocked
    if let Some(current_id) = s.active_account_id.clone() {
        if let Some(user_key) = s.user_key.clone() {
            let session = crate::state::AccountSession {
                user_key,
                org_keys: s.org_keys.clone(),
                rsa_private_key: s.rsa_private_key.clone(),
                master_password_hash: zeroize::Zeroizing::new(s.master_password.to_string()),
                access_token: s.access_token.clone(),
                refresh_token: s.refresh_token.clone(),
                vault_state: s.vault_state,
                http_client: s.http_client.clone().unwrap_or_else(api::build_client),
            };
            s.sessions.insert(current_id, session);
        }
    }

    // Open database for target account
    let store = accounts::load(&s.master_key).map_err(|e| e.to_string())?;
    let _account = store.accounts.iter().find(|a| a.id == profile_id)
        .ok_or_else(|| format!("Account not found: {profile_id}"))?;

    let db_path = accounts::account_db_dir(&profile_id).join("vault.db");
    let conn = vaultwarden_core::storage::db::open(&db_path).map_err(|e| e.to_string())?;
    s.db = Some(conn);
    s.active_account_id = Some(profile_id.clone());
    accounts::set_active_account(&s.master_key, &profile_id).map_err(|e| e.to_string())?;

    // Restore session if already unlocked
    if let Some(session) = s.sessions.get(&profile_id).cloned() {
        s.user_key = Some(session.user_key);
        s.org_keys = session.org_keys;
        s.rsa_private_key = session.rsa_private_key;
        s.access_token = session.access_token;
        s.refresh_token = session.refresh_token;
        s.http_client = Some(session.http_client);
        s.vault_state = session.vault_state;
        Ok("unlocked".into())
    } else {
        s.user_key = None;
        s.org_keys.clear();
        s.rsa_private_key = None;
        s.access_token = None;
        s.refresh_token = None;
        s.http_client = None;
        s.vault_state = vaultwarden_core::sync::VaultState::Locked;
        Ok("locked".into())
    }
}
