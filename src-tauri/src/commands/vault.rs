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

    let result: Result<vaultwarden_core::sync::SyncResult, String> =
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
        .map_err(|e| format!("Sync task panicked: {e}"))?;

    let result = result?;

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

#[derive(Serialize)]
pub struct RevealedSecret {
    pub password: Option<String>,
    pub description: Option<String>,
    pub has_totp: bool,
    pub totp_secret: Option<String>,
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

    if let Some(ref notes_enc) = cipher.notes_enc {
        description = item_key.decrypt_string(notes_enc).ok();
    }

    if let Some(ref login_json) = cipher.login_enc {
        if let Ok(login) = serde_json::from_str::<SyncCipherLogin>(login_json) {
            if let Some(ref pass_enc) = login.password {
                password = item_key.decrypt_string(pass_enc).ok();
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

    Ok(RevealedSecret {
        password,
        description,
        has_totp,
        totp_secret,
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

#[derive(serde::Deserialize)]
pub struct NewResourceInput {
    pub name: String,
    pub username: Option<String>,
    pub uri: Option<String>,
    pub password: String,
    pub description: Option<String>,
    pub folder_parent_id: Option<String>,
}

#[tauri::command]
pub async fn create_resource(
    _input: NewResourceInput,
    _state: State<'_, SafeState>,
) -> Result<String, String> {
    Err("Online create will be synced in Milestone 9".into())
}

#[derive(serde::Deserialize)]
pub struct ResourceUpdateInput {
    pub name: String,
    pub username: Option<String>,
    pub uri: Option<String>,
    pub password: String,
    pub description: Option<String>,
}

#[tauri::command]
pub async fn update_resource(
    _resource_id: String,
    _input: ResourceUpdateInput,
    _state: State<'_, SafeState>,
) -> Result<(), String> {
    Err("Online update will be synced in Milestone 9".into())
}

#[tauri::command]
pub async fn delete_resource(_resource_id: String, _state: State<'_, SafeState>) -> Result<(), String> {
    Err("Online delete will be synced in Milestone 9".into())
}

#[tauri::command]
pub async fn create_folder(
    _name: String,
    _parent_id: Option<String>,
    _state: State<'_, SafeState>,
) -> Result<String, String> {
    Err("Online folder create will be synced in Milestone 9".into())
}

#[tauri::command]
pub async fn rename_folder(
    _folder_id: String,
    _name: String,
    _state: State<'_, SafeState>,
) -> Result<(), String> {
    Err("Online folder rename will be synced in Milestone 9".into())
}

#[tauri::command]
pub async fn delete_folder(_folder_id: String, _state: State<'_, SafeState>) -> Result<(), String> {
    Err("Online folder delete will be synced in Milestone 9".into())
}

#[tauri::command]
pub async fn move_item(
    _model_type: String,
    _item_id: String,
    _target_parent_id: Option<String>,
    _state: State<'_, SafeState>,
) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn toggle_favorite(
    _resource_id: String,
    _state: State<'_, SafeState>,
) -> Result<bool, String> {
    Ok(true)
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
