use reqwest::Client;
use rusqlite::Connection;
use std::collections::HashMap;

use crate::api;
use crate::crypto::SymmetricKey;
use crate::storage::db::{self, CipherItem, CollectionRecord, FolderRecord, OrganizationRecord};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultState {
    Locked,
    OnlineReady,
    OfflineRead,
    Syncing,
}

#[derive(Debug)]
pub struct SyncResult {
    pub state: VaultState,
    pub added: usize,
    pub updated: usize,
    pub deleted: usize,
    pub error: Option<String>,
}

/// Runs a full sync against the Vaultwarden / Bitwarden `/api/sync` endpoint,
/// updating the local SQLite cache and rebuilding the search cache.
pub async fn sync_vault(
    conn: &Connection,
    client: &Client,
    api_url: &str,
    access_token: &str,
    user_key: &SymmetricKey,
    org_keys: &HashMap<String, SymmetricKey>,
) -> SyncResult {
    let sync_data = match api::fetch_sync_data(client, api_url, access_token).await {
        Ok(data) => data,
        Err(e) => {
            return SyncResult {
                state: VaultState::OfflineRead,
                added: 0,
                updated: 0,
                deleted: 0,
                error: Some(format!("Sync failed, offline fallback: {e}")),
            };
        }
    };

    // 1. Process Organizations
    if let Some(orgs) = sync_data.profile.organizations {
        for org in orgs {
            let record = OrganizationRecord {
                id: org.id,
                name: org.name,
                key_enc: org.key.unwrap_or_default(),
                use_totp: false,
            };
            let _ = db::upsert_organization(conn, &record);
        }
    }

    // 2. Process Collections
    if let Some(collections) = sync_data.collections {
        for col in collections {
            let record = CollectionRecord {
                id: col.id,
                organization_id: col.organization_id,
                name_enc: col.name,
                read_only: col.read_only.unwrap_or(false),
            };
            let _ = db::upsert_collection(conn, &record);
        }
    }

    // 3. Process Folders
    if let Some(folders) = sync_data.folders {
        for f in folders {
            let record = FolderRecord {
                id: f.id,
                name_enc: f.name,
                revision_date: f
                    .revision_date
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.timestamp())
                    .unwrap_or(0),
            };
            let _ = db::upsert_folder(conn, &record);
        }
    }

    // 4. Process Ciphers
    let mut added = 0usize;
    let mut updated = 0usize;

    if let Some(ciphers) = sync_data.ciphers {
        for c in ciphers {
            // Check if cipher exists to count added vs updated
            let exists = db::get_cipher_by_id(conn, &c.id).is_ok();

            // Determine key to decrypt name / username / uri for search cache
            let parent_key = if let Some(ref org_id) = c.organization_id {
                org_keys.get(org_id).unwrap_or(user_key)
            } else {
                user_key
            };

            let item_key = if let Some(ref key_enc) = c.key {
                parent_key
                    .decrypt_bytes(key_enc)
                    .ok()
                    .and_then(|b| SymmetricKey::from_slice(&b).ok())
                    .unwrap_or_else(|| parent_key.clone())
            } else {
                parent_key.clone()
            };

            let decrypted_name = item_key.decrypt_string(&c.name).unwrap_or_else(|_| c.name.clone());

            let mut decrypted_username = None;
            let mut decrypted_uri = None;

            if let Some(ref login) = c.login {
                if let Some(ref u_enc) = login.username {
                    decrypted_username = item_key.decrypt_string(u_enc).ok();
                }
                if let Some(ref uris) = login.uris {
                    if let Some(first_uri) = uris.first() {
                        if let Some(ref uri_enc) = first_uri.uri {
                            decrypted_uri = item_key.decrypt_string(uri_enc).ok();
                        }
                    }
                }
            }

            let rev_date = c
                .revision_date
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.timestamp())
                .unwrap_or(0);

            let del_date = c
                .deleted_date
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.timestamp());

            let item = CipherItem {
                id: c.id.clone(),
                organization_id: c.organization_id,
                folder_id: c.folder_id,
                cipher_type: c.cipher_type,
                name_enc: c.name,
                notes_enc: c.notes,
                login_enc: c.login.as_ref().and_then(|l| serde_json::to_string(l).ok()),
                card_enc: c.card.as_ref().and_then(|cd| serde_json::to_string(cd).ok()),
                identity_enc: c.identity.as_ref().and_then(|i| serde_json::to_string(i).ok()),
                fields_enc: c.fields.as_ref().and_then(|f| serde_json::to_string(f).ok()),
                key_enc: c.key,
                favorite: c.favorite.unwrap_or(false),
                revision_date: rev_date,
                deleted_date: del_date,
            };

            if let Err(e) = db::upsert_cipher(
                conn,
                &item,
                &decrypted_name,
                decrypted_username.as_deref(),
                decrypted_uri.as_deref(),
            ) {
                eprintln!("warn: failed to upsert cipher {}: {e}", c.id);
                continue;
            }

            if exists {
                updated += 1;
            } else {
                added += 1;
            }
        }
    }

    SyncResult {
        state: VaultState::OnlineReady,
        added,
        updated,
        deleted: 0,
        error: None,
    }
}
