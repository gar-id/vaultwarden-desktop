use tauri::State;
use serde::Serialize;
use arboard::Clipboard;
use vaultwarden_core::{
    api::{self, ServerEndpoints, SyncCipherLogin},
    crypto,
    storage::{accounts, db},
};
use crate::state::SafeState;

#[derive(Serialize, Clone)]
pub struct ResourceItem {
    pub id: String,
    pub name: String,
    pub username: Option<String>,
    pub uri: Option<String>,
    pub description: Option<String>,
    pub modified_at: i64,
    pub folder_parent_id: Option<String>,
    pub resource_type_slug: Option<String>,
    pub is_favorite: bool,
    pub tags: Vec<String>,
}

#[derive(Serialize)]
pub struct SyncResultDto {
    pub state: String,
    pub added: usize,
    pub updated: usize,
    pub error: Option<String>,
}

/// Runs one full sync pass on a dedicated thread (rusqlite `Connection` is
/// not `Send`, so the connection is opened inside the blocking task).
async fn run_sync(
    account_id: String,
    client: api::HttpClient,
    endpoints: ServerEndpoints,
    access_token: String,
    user_key: crypto::SymmetricKey,
    org_keys: std::collections::HashMap<String, crypto::SymmetricKey>,
) -> Result<vaultwarden_core::sync::SyncResult, String> {
    tokio::task::spawn_blocking(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = tx.send(Err(format!("Failed to start sync runtime: {e}")));
                    return;
                }
            };
            let outcome = runtime.block_on(async {
                let db_path = accounts::account_db_dir(&account_id).join("vault.db");
                let conn = db::open(&db_path).map_err(|e| e.to_string())?;
                Ok(vaultwarden_core::sync::sync_vault(
                    &conn, &client.client, &endpoints.api_url, &access_token, &user_key, &org_keys,
                ).await)
            });
            let _ = tx.send(outcome);
        });
        rx.recv().map_err(|_| "Sync thread panicked".to_string())?
    })
    .await
    .map_err(|e| format!("Sync task panicked: {e}"))?
}

#[tauri::command]
pub async fn sync_vault(state: State<'_, SafeState>) -> Result<SyncResultDto, String> {
    log::info!("sync_vault: called");

    let (account_id, server_url, access_token, user_key, org_keys, client) = {
        let s = state.lock().unwrap();
        let account_id = s.active_account_id.clone().ok_or("No active account")?;
        let user_key = s.user_key.clone().ok_or("Vault is locked — cannot sync")?;
        let client = s.http_client.clone().unwrap_or_else(api::build_client);
        let access_token = s.access_token.clone().unwrap_or_default();

        let store = accounts::load(&s.master_key).map_err(|e| e.to_string())?;
        let account = store.accounts.iter().find(|a| a.id == account_id)
            .ok_or("Active account not found")?;

        (account_id, account.server_url.clone(), access_token, user_key, s.org_keys.clone(), client)
    };

    let endpoints = ServerEndpoints::from_base_url(&server_url);

    let mut result = run_sync(
        account_id.clone(), client.clone(), endpoints.clone(), access_token,
        user_key.clone(), org_keys.clone(),
    )
    .await?;

    // Access tokens live ~2h; refresh once and retry rather than leaving the
    // user with an offline vault until they lock and unlock again.
    if result.error.as_deref().is_some_and(|e| e.contains("Session expired")) {
        if let Ok(token) = refresh_active_token(&state).await {
            log::info!("sync_vault: token expired — refreshed, retrying sync");
            result = run_sync(account_id, client, endpoints, token, user_key, org_keys).await?;
        }
    }

    log::info!(
        "sync_vault: finished — state={:?} added={} updated={} error={:?}",
        result.state, result.added, result.updated, result.error
    );

    {
        let mut s = state.lock().unwrap();
        s.vault_state = result.state;
    }

    Ok(SyncResultDto {
        state: match result.state {
            vaultwarden_core::sync::VaultState::OnlineReady => "online",
            vaultwarden_core::sync::VaultState::OfflineRead => "offline",
            vaultwarden_core::sync::VaultState::Syncing => "syncing",
            vaultwarden_core::sync::VaultState::Locked => "locked",
        }.into(),
        added: result.added,
        updated: result.updated,
        error: result.error,
    })
}

#[tauri::command]
pub async fn force_resync(state: State<'_, SafeState>) -> Result<SyncResultDto, String> {
    log::info!("force_resync: called");
    sync_vault(state).await
}

#[tauri::command]
pub async fn clear_local_cache(state: State<'_, SafeState>) -> Result<SyncResultDto, String> {
    log::info!("clear_local_cache: called");
    let account_id = {
        let s = state.lock().unwrap();
        s.active_account_id.clone().ok_or("No active account")?
    };
    let db_path = accounts::account_db_dir(&account_id).join("vault.db");
    if let Ok(conn) = db::open(&db_path) {
        let _ = conn.execute("DELETE FROM ciphers", []);
        let _ = conn.execute("DELETE FROM search_cache", []);
        let _ = conn.execute("DELETE FROM folders", []);
        let _ = conn.execute("DELETE FROM collections", []);
    }
    sync_vault(state).await
}

#[tauri::command]
pub fn search_resources(
    query: String,
    folder_id: Option<String>,
    state: State<'_, SafeState>,
) -> Result<Vec<ResourceItem>, String> {
    let s = state.lock().unwrap();
    let conn = s.db.as_ref().ok_or("No active vault database")?;
    let folder_filter = folder_id.as_deref().filter(|f| !f.is_empty());

    let results = db::search_ciphers(conn, &query, folder_filter, None).map_err(|e| e.to_string())?;

    Ok(results.into_iter().map(|c| {
        let type_slug = match c.cipher_type {
            1 => "login",
            2 => "note",
            3 => "card",
            4 => "identity",
            _ => "login",
        };
        ResourceItem {
            id: c.id,
            name: c.name,
            username: c.username,
            uri: c.uri,
            description: None,
            modified_at: c.revision_date,
            folder_parent_id: c.folder_id,
            resource_type_slug: Some(type_slug.into()),
            is_favorite: c.favorite,
            tags: vec![],
        }
    }).collect())
}

#[derive(Serialize)]
pub struct FolderNode {
    pub id: String,
    pub name: String,
    pub children: Vec<FolderNode>,
    pub resource_count: u32,
}

#[tauri::command]
pub fn list_folder_tree(state: State<'_, SafeState>) -> Result<Vec<FolderNode>, String> {
    let s = state.lock().unwrap();
    let conn = s.db.as_ref().ok_or("No active vault database")?;
    let user_key = s.user_key.as_ref().ok_or("Vault is locked")?;

    let flat = db::list_folders(conn).map_err(|e| e.to_string())?;

    let mut nodes = Vec::new();
    for f in flat {
        let name = user_key.decrypt_string(&f.name_enc).unwrap_or(f.name_enc);
        let count: u32 = conn.query_row(
            "SELECT COUNT(*) FROM ciphers WHERE folder_id = ?1 AND deleted_date IS NULL",
            [&f.id],
            |row| row.get(0),
        ).unwrap_or(0);

        nodes.push(FolderNode {
            id: f.id,
            name,
            children: vec![],
            resource_count: count,
        });
    }

    Ok(nodes)
}

#[derive(Serialize, serde::Deserialize, Clone, Debug)]
pub struct DecryptedField {
    pub name: Option<String>,
    pub value: Option<String>,
    #[serde(rename = "type")]
    pub field_type: i32,
}

#[derive(Serialize, serde::Deserialize, Clone, Debug)]
pub struct DecryptedPasswordHistory {
    pub last_used_date: Option<String>,
    pub password: Option<String>,
}

#[derive(Serialize)]
pub struct RevealedSecret {
    pub password: Option<String>,
    pub description: Option<String>,
    pub has_totp: bool,
    pub totp_secret: Option<String>,
    pub uris: Vec<String>,
    pub fields: Vec<DecryptedField>,
    pub password_history: Vec<DecryptedPasswordHistory>,
}

fn get_item_key(s: &crate::state::AppState, cipher: &db::CipherItem) -> Result<crypto::SymmetricKey, String> {
    let user_key = s.user_key.as_ref().ok_or("Vault is locked")?;
    let parent_key = if let Some(ref org_id) = cipher.organization_id {
        s.org_keys.get(org_id).unwrap_or(user_key)
    } else {
        user_key
    };

    if let Some(ref key_enc) = cipher.key_enc {
        parent_key
            .decrypt_bytes(key_enc)
            .map_err(|e| e.to_string())
            .and_then(|b| crypto::SymmetricKey::from_slice(&b).map_err(|e| e.to_string()))
    } else {
        Ok(parent_key.clone())
    }
}

/// Encrypts each non-empty URL/host for the `login.uris` array.
fn encrypt_uris(
    key: &crypto::SymmetricKey,
    uris: &[String],
) -> Result<Option<Vec<api::SyncCipherUri>>, String> {
    let mut out = Vec::new();
    for uri in uris {
        let uri = uri.trim();
        if uri.is_empty() {
            continue;
        }
        out.push(api::SyncCipherUri {
            uri: Some(key.encrypt_string(uri).map_err(|e| e.to_string())?),
            match_type: None,
        });
    }
    Ok(if out.is_empty() { None } else { Some(out) })
}

#[tauri::command]
pub fn reveal_secret(resource_id: String, state: State<'_, SafeState>) -> Result<RevealedSecret, String> {
    let s = state.lock().unwrap();
    let conn = s.db.as_ref().ok_or("No active vault database")?;
    let cipher = db::get_cipher_by_id(conn, &resource_id).map_err(|e| e.to_string())?;

    let item_key = get_item_key(&s, &cipher)?;

    let mut password = None;
    let mut description = None;
    let mut has_totp = false;
    let mut totp_secret = None;
    let mut uris = Vec::new();

    if let Some(ref notes_enc) = cipher.notes_enc {
        description = item_key.decrypt_string(notes_enc).ok();
    }

    if let Some(ref login_json) = cipher.login_enc {
        if let Ok(login) = serde_json::from_str::<SyncCipherLogin>(login_json) {
            if let Some(ref pass_enc) = login.password {
                password = item_key.decrypt_string(pass_enc).ok();
            }
            if let Some(ref login_uris) = login.uris {
                for u in login_uris {
                    if let Some(uri) = u.uri.as_deref().and_then(|e| item_key.decrypt_string(e).ok()) {
                        uris.push(uri);
                    }
                }
            }
            if let Some(ref totp_enc) = login.totp {
                if let Ok(t) = item_key.decrypt_string(totp_enc) {
                    if !t.is_empty() {
                        has_totp = true;
                        totp_secret = Some(t);
                    }
                }
            }
        }
    }

    let mut fields = Vec::new();
    if let Some(ref fields_json) = cipher.fields_enc {
        if let Ok(raw_fields) = serde_json::from_str::<Vec<api::SyncCipherField>>(fields_json) {
            for f in raw_fields {
                let name = f.name.as_ref().map(|n| item_key.decrypt_string(n).unwrap_or_else(|_| n.clone()));
                let value = f.value.as_ref().map(|v| item_key.decrypt_string(v).unwrap_or_else(|_| v.clone()));
                fields.push(DecryptedField {
                    name,
                    value,
                    field_type: f.field_type.unwrap_or(0),
                });
            }
        }
    }

    let mut password_history = Vec::new();
    if let Some(ref hist_json) = cipher.password_history_enc {
        if let Ok(raw_hist) = serde_json::from_str::<Vec<api::SyncPasswordHistory>>(hist_json) {
            for h in raw_hist {
                let pass = h.password.as_ref().map(|p| item_key.decrypt_string(p).unwrap_or_else(|_| p.clone()));
                password_history.push(DecryptedPasswordHistory {
                    last_used_date: h.last_used_date,
                    password: pass,
                });
            }
        }
    }

    Ok(RevealedSecret {
        password,
        description,
        has_totp,
        totp_secret,
        uris,
        fields,
        password_history,
    })
}

#[derive(Serialize)]
pub struct TotpCode {
    pub code: String,
    pub expires_in_secs: u32,
}

#[tauri::command]
pub fn get_totp_code(resource_id: String, state: State<'_, SafeState>) -> Result<TotpCode, String> {
    let secret = {
        let revealed = reveal_secret(resource_id, state)?;
        revealed.totp_secret.ok_or("No TOTP configured for this item")?
    };

    let res = crypto::generate_totp_code(&secret).map_err(|e| e.to_string())?;
    Ok(TotpCode {
        code: res.code,
        expires_in_secs: res.expires_in_secs,
    })
}

#[tauri::command]
pub fn copy_secret(resource_id: String, state: State<'_, SafeState>) -> Result<(), String> {
    let revealed = reveal_secret(resource_id, state)?;
    let text_to_copy = revealed.password.ok_or("No password to copy")?;

    std::thread::spawn(move || {
        let mut clipboard = match Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("copy_secret: failed to open clipboard: {e}");
                return;
            }
        };
        if let Err(e) = clipboard.set_text(&text_to_copy) {
            log::warn!("copy_secret: failed to set clipboard text: {e}");
            return;
        }
        let clear_secs = crate::commands::crypto::CLIPBOARD_CLEAR_SECS.load(std::sync::atomic::Ordering::Relaxed);
        if clear_secs > 0 {
            std::thread::sleep(std::time::Duration::from_secs(clear_secs));
            let _ = clipboard.set_text("");
        } else {
            std::thread::sleep(std::time::Duration::from_secs(300));
        }
    });

    Ok(())
}

#[derive(serde::Deserialize)]
pub struct PasswordRules {
    pub length: usize,
    pub uppercase: bool,
    pub lowercase: bool,
    pub numbers: bool,
    pub symbols: bool,
}

#[tauri::command]
pub fn generate_password(rules: PasswordRules) -> Result<String, String> {
    use rand::seq::SliceRandom;

    let mut charset: Vec<char> = Vec::new();
    if rules.uppercase { charset.extend('A'..='Z'); }
    if rules.lowercase { charset.extend('a'..='z'); }
    if rules.numbers   { charset.extend('0'..='9'); }
    if rules.symbols   { charset.extend("!@#$%^&*()-_=+[]{}|;:,.<>?".chars()); }
    if charset.is_empty() {
        return Err("Select at least one character class".into());
    }

    let length = rules.length.clamp(8, 128);
    let mut rng = rand::thread_rng();
    let password: String = (0..length)
        .map(|_| *charset.choose(&mut rng).unwrap())
        .collect();
    Ok(password)
}

struct ActiveSessionContext {
    user_key: crypto::SymmetricKey,
    org_keys: std::collections::HashMap<String, crypto::SymmetricKey>,
    client: api::HttpClient,
    endpoints: ServerEndpoints,
    user_id: Option<String>,
}

fn get_active_context(state: &SafeState) -> Result<ActiveSessionContext, String> {
    let s = state.lock().unwrap();
    let account_id = s.active_account_id.clone().ok_or("No active account")?;
    let user_key = s.user_key.clone().ok_or("Vault is locked")?;
    let client = s.http_client.clone().unwrap_or_else(api::build_client);

    let store = accounts::load(&s.master_key).map_err(|e| e.to_string())?;
    let account = store
        .accounts
        .iter()
        .find(|a| a.id == account_id)
        .ok_or("Active account not found")?;

    let user_id = if let Some(ref conn) = s.db {
        db::get_metadata(conn, "user_id").ok().flatten()
    } else {
        None
    };

    let endpoints = ServerEndpoints::from_base_url(&account.server_url);

    Ok(ActiveSessionContext {
        user_key,
        org_keys: s.org_keys.clone(),
        client,
        endpoints,
        user_id,
    })
}

/// Exchanges the active account's refresh token for a new access token and
/// persists it (memory, session map, and the encrypted account store).
async fn refresh_active_token(state: &SafeState) -> Result<String, String> {
    let (client, identity_url, refresh_token, account_id) = {
        let s = state.lock().unwrap();
        let account_id = s.active_account_id.clone().ok_or("No active account")?;
        let store = accounts::load(&s.master_key).map_err(|e| e.to_string())?;
        let account = store
            .accounts
            .iter()
            .find(|a| a.id == account_id)
            .ok_or("Active account not found")?;
        let refresh_token = s
            .refresh_token
            .clone()
            .or_else(|| account.refresh_token.clone())
            .ok_or("Session expired — please unlock the vault again")?;
        let endpoints = ServerEndpoints::from_base_url(&account.server_url);
        (
            s.http_client.clone().unwrap_or_else(api::build_client),
            endpoints.identity_url,
            refresh_token,
            account_id,
        )
    };

    let tok = api::refresh_access_token(&client.client, &identity_url, &refresh_token)
        .await
        .map_err(|_| "Session expired — please unlock the vault again".to_string())?;

    {
        let mut s = state.lock().unwrap();
        s.access_token = Some(tok.access_token.clone());
        if tok.refresh_token.is_some() {
            s.refresh_token = tok.refresh_token.clone();
        }
        if let Some(session) = s.sessions.get_mut(&account_id) {
            session.access_token = Some(tok.access_token.clone());
            if tok.refresh_token.is_some() {
                session.refresh_token = tok.refresh_token.clone();
            }
        }
        let mut store = accounts::load(&s.master_key).map_err(|e| e.to_string())?;
        if let Some(account) = store.accounts.iter_mut().find(|a| a.id == account_id) {
            account.access_token = Some(tok.access_token.clone());
            if tok.refresh_token.is_some() {
                account.refresh_token = tok.refresh_token.clone();
            }
            accounts::save(&s.master_key, &store).map_err(|e| e.to_string())?;
        }
    }

    Ok(tok.access_token)
}

/// Runs an authenticated API call. If the server reports the access token has
/// expired, refreshes it and retries the call exactly once.
macro_rules! authed_call {
    ($state:expr, $token:ident, $call:expr) => {{
        let mut $token = { $state.lock().unwrap().access_token.clone().unwrap_or_default() };
        match $call.await {
            Err(api::ApiError::SessionExpired) => {
                log::info!("access token expired — refreshing and retrying");
                $token = refresh_active_token(&$state).await?;
                $call.await
            }
            other => other,
        }
        .map_err(|e| e.to_string())?
    }};
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct CustomFieldInput {
    pub name: String,
    pub value: String,
    #[serde(rename = "type")]
    pub field_type: i32,
}

#[derive(serde::Deserialize)]
pub struct NewResourceInput {
    pub name: String,
    pub username: Option<String>,
    #[serde(default)]
    pub uris: Option<Vec<String>>,
    pub password: String,
    pub description: Option<String>,
    pub folder_parent_id: Option<String>,
    #[serde(default)]
    pub totp_secret: Option<String>,
    #[serde(default)]
    pub fields: Vec<CustomFieldInput>,
}

#[tauri::command]
pub async fn create_resource(
    input: NewResourceInput,
    state: State<'_, SafeState>,
) -> Result<String, String> {
    let ctx = get_active_context(&state)?;

    let enc_name = ctx.user_key.encrypt_string(&input.name).map_err(|e| e.to_string())?;
    let enc_notes = match input.description.as_deref().filter(|d| !d.is_empty()) {
        Some(d) => Some(ctx.user_key.encrypt_string(d).map_err(|e| e.to_string())?),
        None => None,
    };

    let enc_username = match input.username.as_deref().filter(|u| !u.is_empty()) {
        Some(u) => Some(ctx.user_key.encrypt_string(u).map_err(|e| e.to_string())?),
        None => None,
    };
    let enc_password = if !input.password.is_empty() {
        Some(ctx.user_key.encrypt_string(&input.password).map_err(|e| e.to_string())?)
    } else {
        None
    };
    let enc_uris = match input.uris.as_deref() {
        Some(uris) => encrypt_uris(&ctx.user_key, uris)?,
        None => None,
    };

    let enc_totp = match input.totp_secret.as_deref().filter(|t| !t.is_empty()) {
        Some(t) => Some(ctx.user_key.encrypt_string(t).map_err(|e| e.to_string())?),
        None => None,
    };

    let mut enc_fields = Vec::new();
    for f in &input.fields {
        let f_name = ctx.user_key.encrypt_string(&f.name).map_err(|e| e.to_string())?;
        let f_val = ctx.user_key.encrypt_string(&f.value).map_err(|e| e.to_string())?;
        enc_fields.push(api::SyncCipherField {
            name: Some(f_name),
            value: Some(f_val),
            field_type: Some(f.field_type),
            linked_id: None,
        });
    }

    let folder_id = input.folder_parent_id.filter(|f| f != "__favorites__" && !f.is_empty());

    let req = api::CipherRequest {
        cipher_type: 1, // Login
        folder_id,
        organization_id: None,
        name: enc_name,
        notes: enc_notes,
        favorite: false,
        reprompt: 0,
        encrypted_for: ctx.user_id,
        login: Some(api::SyncCipherLogin {
            username: enc_username,
            password: enc_password,
            totp: enc_totp,
            uris: enc_uris,
        }),
        card: None,
        identity: None,
        fields: if enc_fields.is_empty() { None } else { Some(enc_fields) },
        password_history: None,
        key: None,
    };

    let created_cipher = authed_call!(state, token, api::create_cipher(
        &ctx.client.client,
        &ctx.endpoints.api_url,
        &token,
        &req,
    ));

    {
        let s = state.lock().unwrap();
        if let Some(ref conn) = s.db {
            vaultwarden_core::sync::save_sync_cipher(conn, &created_cipher, &ctx.user_key, &ctx.org_keys)
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(created_cipher.id)
}

#[derive(serde::Deserialize)]
pub struct ResourceUpdateInput {
    pub name: String,
    pub username: Option<String>,
    #[serde(default)]
    pub uris: Option<Vec<String>>,
    pub password: String,
    pub description: Option<String>,
    pub folder_parent_id: Option<String>,
    #[serde(default)]
    pub totp_secret: Option<String>,
    #[serde(default)]
    pub fields: Vec<CustomFieldInput>,
}

#[tauri::command]
pub async fn update_resource(
    resource_id: String,
    input: ResourceUpdateInput,
    state: State<'_, SafeState>,
) -> Result<(), String> {
    let ctx = get_active_context(&state)?;

    let (cipher, item_key) = {
        let s = state.lock().unwrap();
        let conn = s.db.as_ref().ok_or("No active vault database")?;
        let cipher = db::get_cipher_by_id(conn, &resource_id).map_err(|e| e.to_string())?;
        let item_key = get_item_key(&s, &cipher)?;
        (cipher, item_key)
    };

    let enc_name = item_key.encrypt_string(&input.name).map_err(|e| e.to_string())?;
    let enc_notes = match input.description.as_deref().filter(|d| !d.is_empty()) {
        Some(d) => Some(item_key.encrypt_string(d).map_err(|e| e.to_string())?),
        None => None,
    };

    let enc_username = match input.username.as_deref().filter(|u| !u.is_empty()) {
        Some(u) => Some(item_key.encrypt_string(u).map_err(|e| e.to_string())?),
        None => None,
    };

    let existing_login: Option<api::SyncCipherLogin> = cipher
        .login_enc
        .as_deref()
        .and_then(|l| serde_json::from_str(l).ok());

    let existing_password_decrypted = existing_login
        .as_ref()
        .and_then(|l| l.password.as_deref())
        .and_then(|p| item_key.decrypt_string(p).ok());

    let mut history: Vec<api::SyncPasswordHistory> = cipher
        .password_history_enc
        .as_deref()
        .and_then(|h| serde_json::from_str(h).ok())
        .unwrap_or_default();

    if !input.password.is_empty()
        && existing_password_decrypted.as_deref() != Some(&input.password)
        && existing_login.as_ref().and_then(|l| l.password.as_ref()).is_some()
    {
        history.push(api::SyncPasswordHistory {
            last_used_date: Some(chrono::Utc::now().to_rfc3339()),
            password: existing_login.as_ref().and_then(|l| l.password.clone()),
        });
    }

    let enc_password = if !input.password.is_empty() {
        Some(item_key.encrypt_string(&input.password).map_err(|e| e.to_string())?)
    } else {
        existing_login.as_ref().and_then(|l| l.password.clone())
    };

    // An explicitly supplied list replaces the old one (an empty list clears
    // it); omitting the field keeps whatever the cipher already had.
    let enc_uris = match input.uris.as_deref() {
        Some(uris) => encrypt_uris(&item_key, uris)?,
        None => existing_login.as_ref().and_then(|l| l.uris.clone()),
    };

    // An empty/omitted value clears the stored authenticator secret.
    let enc_totp = match input.totp_secret.as_deref().filter(|t| !t.is_empty()) {
        Some(t) => Some(item_key.encrypt_string(t).map_err(|e| e.to_string())?),
        None => None,
    };

    let mut enc_fields = Vec::new();
    for f in &input.fields {
        let f_name = item_key.encrypt_string(&f.name).map_err(|e| e.to_string())?;
        let f_val = item_key.encrypt_string(&f.value).map_err(|e| e.to_string())?;
        enc_fields.push(api::SyncCipherField {
            name: Some(f_name),
            value: Some(f_val),
            field_type: Some(f.field_type),
            linked_id: None,
        });
    }

    let folder_id = input
        .folder_parent_id
        .filter(|f| f != "__favorites__" && !f.is_empty());

    let req = api::CipherRequest {
        cipher_type: cipher.cipher_type,
        folder_id,
        organization_id: cipher.organization_id.clone(),
        name: enc_name,
        notes: enc_notes,
        favorite: cipher.favorite,
        reprompt: 0,
        encrypted_for: ctx.user_id,
        login: if cipher.cipher_type == 1 {
            Some(api::SyncCipherLogin {
                username: enc_username,
                password: enc_password,
                totp: enc_totp,
                uris: enc_uris,
            })
        } else {
            None
        },
        card: cipher.card_enc.as_deref().and_then(|c| serde_json::from_str(c).ok()),
        identity: cipher.identity_enc.as_deref().and_then(|i| serde_json::from_str(i).ok()),
        fields: if enc_fields.is_empty() { None } else { Some(enc_fields) },
        password_history: if history.is_empty() { None } else { Some(history) },
        key: cipher.key_enc,
    };

    let updated_cipher = authed_call!(state, token, api::update_cipher(
        &ctx.client.client,
        &ctx.endpoints.api_url,
        &token,
        &resource_id,
        &req,
    ));

    {
        let s = state.lock().unwrap();
        if let Some(ref conn) = s.db {
            vaultwarden_core::sync::save_sync_cipher(conn, &updated_cipher, &ctx.user_key, &ctx.org_keys)
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn delete_resource(resource_id: String, state: State<'_, SafeState>) -> Result<(), String> {
    let ctx = get_active_context(&state)?;

    authed_call!(state, token, api::delete_cipher(
        &ctx.client.client,
        &ctx.endpoints.api_url,
        &token,
        &resource_id,
    ));

    {
        let s = state.lock().unwrap();
        if let Some(ref conn) = s.db {
            db::delete_cipher(conn, &resource_id).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn create_folder(
    name: String,
    parent_id: Option<String>,
    state: State<'_, SafeState>,
) -> Result<String, String> {
    let ctx = get_active_context(&state)?;

    let folder_name = if let Some(ref pid) = parent_id {
        let parent_enc: Option<String> = {
            let s = state.lock().unwrap();
            s.db.as_ref().and_then(|conn| {
                conn.query_row("SELECT name_enc FROM folders WHERE id = ?1", [pid], |r| r.get(0)).ok()
            })
        };
        if let Some(p_enc) = parent_enc {
            let p_name = ctx.user_key.decrypt_string(&p_enc).unwrap_or(p_enc);
            format!("{p_name}/{name}")
        } else {
            name
        }
    } else {
        name
    };

    let enc_name = ctx.user_key.encrypt_string(&folder_name).map_err(|e| e.to_string())?;

    let created = authed_call!(state, token, api::create_folder(
        &ctx.client.client,
        &ctx.endpoints.api_url,
        &token,
        &enc_name,
    ));

    let rev_date = created
        .revision_date
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp())
        .unwrap_or_else(|| chrono::Utc::now().timestamp());

    {
        let s = state.lock().unwrap();
        if let Some(ref conn) = s.db {
            db::upsert_folder(
                conn,
                &db::FolderRecord {
                    id: created.id.clone(),
                    name_enc: created.name.clone(),
                    revision_date: rev_date,
                },
            )
            .map_err(|e| e.to_string())?;
        }
    }

    Ok(created.id)
}

#[tauri::command]
pub async fn rename_folder(
    folder_id: String,
    name: String,
    state: State<'_, SafeState>,
) -> Result<(), String> {
    let ctx = get_active_context(&state)?;

    let enc_name = ctx.user_key.encrypt_string(&name).map_err(|e| e.to_string())?;

    let updated = authed_call!(state, token, api::update_folder(
        &ctx.client.client,
        &ctx.endpoints.api_url,
        &token,
        &folder_id,
        &enc_name,
    ));

    let rev_date = updated
        .revision_date
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp())
        .unwrap_or_else(|| chrono::Utc::now().timestamp());

    {
        let s = state.lock().unwrap();
        if let Some(ref conn) = s.db {
            db::upsert_folder(
                conn,
                &db::FolderRecord {
                    id: folder_id,
                    name_enc: updated.name,
                    revision_date: rev_date,
                },
            )
            .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn delete_folder(folder_id: String, state: State<'_, SafeState>) -> Result<(), String> {
    let ctx = get_active_context(&state)?;

    authed_call!(state, token, api::delete_folder(
        &ctx.client.client,
        &ctx.endpoints.api_url,
        &token,
        &folder_id,
    ));

    {
        let s = state.lock().unwrap();
        if let Some(ref conn) = s.db {
            db::delete_folder(conn, &folder_id).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn move_item(
    model_type: String,
    item_id: String,
    target_parent_id: Option<String>,
    state: State<'_, SafeState>,
) -> Result<(), String> {
    if model_type != "resource" {
        return Ok(());
    }

    let ctx = get_active_context(&state)?;

    let cipher = {
        let s = state.lock().unwrap();
        let conn = s.db.as_ref().ok_or("No active vault database")?;
        db::get_cipher_by_id(conn, &item_id).map_err(|e| e.to_string())?
    };

    let target_folder = target_parent_id.filter(|f| f != "__favorites__" && !f.is_empty());

    let req = api::CipherRequest {
        cipher_type: cipher.cipher_type,
        folder_id: target_folder,
        organization_id: cipher.organization_id.clone(),
        name: cipher.name_enc.clone(),
        notes: cipher.notes_enc.clone(),
        favorite: cipher.favorite,
        reprompt: 0,
        encrypted_for: ctx.user_id,
        login: cipher.login_enc.as_deref().and_then(|l| serde_json::from_str(l).ok()),
        card: cipher.card_enc.as_deref().and_then(|c| serde_json::from_str(c).ok()),
        identity: cipher.identity_enc.as_deref().and_then(|i| serde_json::from_str(i).ok()),
        fields: cipher.fields_enc.as_deref().and_then(|f| serde_json::from_str(f).ok()),
        password_history: cipher.password_history_enc.as_deref().and_then(|h| serde_json::from_str(h).ok()),
        key: cipher.key_enc.clone(),
    };

    let updated = authed_call!(state, token, api::update_cipher(
        &ctx.client.client,
        &ctx.endpoints.api_url,
        &token,
        &item_id,
        &req,
    ));

    {
        let s = state.lock().unwrap();
        if let Some(ref conn) = s.db {
            vaultwarden_core::sync::save_sync_cipher(conn, &updated, &ctx.user_key, &ctx.org_keys)
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn toggle_favorite(
    resource_id: String,
    state: State<'_, SafeState>,
) -> Result<bool, String> {
    let ctx = get_active_context(&state)?;

    let cipher = {
        let s = state.lock().unwrap();
        let conn = s.db.as_ref().ok_or("No active vault database")?;
        db::get_cipher_by_id(conn, &resource_id).map_err(|e| e.to_string())?
    };

    let new_fav = !cipher.favorite;

    let req = api::CipherRequest {
        cipher_type: cipher.cipher_type,
        folder_id: cipher.folder_id.clone(),
        organization_id: cipher.organization_id.clone(),
        name: cipher.name_enc.clone(),
        notes: cipher.notes_enc.clone(),
        favorite: new_fav,
        reprompt: 0,
        encrypted_for: ctx.user_id,
        login: cipher.login_enc.as_deref().and_then(|l| serde_json::from_str(l).ok()),
        card: cipher.card_enc.as_deref().and_then(|c| serde_json::from_str(c).ok()),
        identity: cipher.identity_enc.as_deref().and_then(|i| serde_json::from_str(i).ok()),
        fields: cipher.fields_enc.as_deref().and_then(|f| serde_json::from_str(f).ok()),
        password_history: cipher.password_history_enc.as_deref().and_then(|h| serde_json::from_str(h).ok()),
        key: cipher.key_enc.clone(),
    };

    let updated = authed_call!(state, token, api::update_cipher(
        &ctx.client.client,
        &ctx.endpoints.api_url,
        &token,
        &resource_id,
        &req,
    ));

    {
        let s = state.lock().unwrap();
        if let Some(ref conn) = s.db {
            vaultwarden_core::sync::save_sync_cipher(conn, &updated, &ctx.user_key, &ctx.org_keys)
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(new_fav)
}

#[tauri::command]
pub fn list_vault_tags(_state: State<'_, SafeState>) -> Result<Vec<String>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn update_resource_tags(
    _resource_id: String,
    _tags: Vec<String>,
    _state: State<'_, SafeState>,
) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn get_resource_comments(
    _resource_id: String,
    _state: State<'_, SafeState>,
) -> Result<Vec<serde_json::Value>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn add_resource_comment(
    _resource_id: String,
    _content: String,
    _state: State<'_, SafeState>,
) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({}))
}

#[tauri::command]
pub async fn delete_resource_comment(
    _comment_id: String,
    _state: State<'_, SafeState>,
) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn get_resource_activity(
    _resource_id: String,
    _state: State<'_, SafeState>,
) -> Result<Vec<serde_json::Value>, String> {
    Ok(vec![])
}

#[derive(Serialize)]
pub struct SearchAroResultDto {
    pub id: String,
    pub label: String,
    pub sublabel: String,
    pub kind: String,
    pub has_key: bool,
}

#[tauri::command]
pub async fn search_share_targets(
    _query: String,
    _state: State<'_, SafeState>,
) -> Result<Vec<SearchAroResultDto>, String> {
    Ok(vec![])
}

#[derive(Serialize)]
pub struct ItemPermissionDto {
    pub id: Option<String>,
    pub aro: String,
    pub aro_foreign_key: String,
    pub name: String,
    pub identifier: String,
    pub permission_type: i32,
}

#[tauri::command]
pub async fn get_item_permissions(
    _model_type: String,
    _item_id: String,
    _state: State<'_, SafeState>,
) -> Result<Vec<ItemPermissionDto>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn share_item(
    _model_type: String,
    _item_id: String,
    _changes: Vec<serde_json::Value>,
    _state: State<'_, SafeState>,
) -> Result<(), String> {
    Ok(())
}
