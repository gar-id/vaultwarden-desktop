use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::storage::error::StorageError;

#[cfg(test)]
static CONFIG_DIR_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub server_url: String,
    pub email: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub protected_key: Option<String>,
    pub protected_private_key: Option<String>,
    pub last_sync_timestamp: i64,
    pub auto_lock_minutes: u32,
    pub device_id: String,
    #[serde(default)]
    pub kdf_type: Option<u32>,
    #[serde(default)]
    pub kdf_iterations: Option<u32>,
    #[serde(default)]
    pub kdf_memory: Option<u32>,
    #[serde(default)]
    pub kdf_parallelism: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountStore {
    pub active_account_id: Option<String>,
    pub accounts: Vec<Account>,
}

const CONFIG_DIR_OVERRIDE: &str = "VAULTWARDEN_DESKTOP_CONFIG_DIR";

pub fn config_root() -> PathBuf {
    if let Ok(dir) = std::env::var(CONFIG_DIR_OVERRIDE) {
        return PathBuf::from(dir);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("vaultwarden-desktop")
}

pub fn accounts_path() -> PathBuf {
    config_root().join("accounts.enc")
}

pub fn account_db_dir(account_id: &str) -> PathBuf {
    config_root().join("accounts").join(account_id)
}

pub fn load(master_key: &[u8; 32]) -> Result<AccountStore, StorageError> {
    let path = accounts_path();
    if !path.exists() {
        return Ok(AccountStore::default());
    }
    let ciphertext = std::fs::read(&path)?;
    let plaintext = decrypt(master_key, &ciphertext)?;
    Ok(serde_json::from_slice(&plaintext)?)
}

pub fn save(master_key: &[u8; 32], store: &AccountStore) -> Result<(), StorageError> {
    let path = accounts_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let plaintext = serde_json::to_vec(store)?;
    let ciphertext = encrypt(master_key, &plaintext)?;
    std::fs::write(&path, ciphertext)?;
    Ok(())
}

fn encrypt(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, StorageError> {
    let cipher = Aes256Gcm::new(key.into());
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let mut ct = cipher.encrypt(nonce, data).map_err(|_| StorageError::Encryption)?;
    let mut out = nonce_bytes.to_vec();
    out.append(&mut ct);
    Ok(out)
}

fn decrypt(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, StorageError> {
    if data.len() < 12 {
        return Err(StorageError::Decryption);
    }
    let cipher = Aes256Gcm::new(key.into());
    let (nonce_bytes, ct) = data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher.decrypt(nonce, ct).map_err(|_| StorageError::Decryption)
}

pub fn add_account(master_key: &[u8; 32], account: Account) -> Result<(), StorageError> {
    let mut store = load(master_key)?;
    store.accounts.push(account);
    save(master_key, &store)
}

pub fn remove_account(master_key: &[u8; 32], account_id: &str) -> Result<(), StorageError> {
    let mut store = load(master_key)?;
    let before = store.accounts.len();
    store.accounts.retain(|a| a.id != account_id);
    if store.accounts.len() == before {
        return Err(StorageError::NotFound(account_id.into()));
    }
    if store.active_account_id.as_deref() == Some(account_id) {
        store.active_account_id = store.accounts.first().map(|a| a.id.clone());
    }
    save(master_key, &store)
}

pub fn update_account(master_key: &[u8; 32], updated: Account) -> Result<(), StorageError> {
    let mut store = load(master_key)?;
    let a = store
        .accounts
        .iter_mut()
        .find(|a| a.id == updated.id)
        .ok_or_else(|| StorageError::NotFound(updated.id.clone()))?;
    *a = updated;
    save(master_key, &store)
}

pub fn set_active_account(master_key: &[u8; 32], account_id: &str) -> Result<(), StorageError> {
    let mut store = load(master_key)?;
    if !store.accounts.iter().any(|a| a.id == account_id) {
        return Err(StorageError::NotFound(account_id.into()));
    }
    store.active_account_id = Some(account_id.to_string());
    save(master_key, &store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_key() -> [u8; 32] {
        [0xABu8; 32]
    }

    #[test]
    fn test_accounts_crud_roundtrip() {
        let _guard = CONFIG_DIR_ENV_LOCK.lock().unwrap();
        let dir = tempdir().unwrap();
        std::env::set_var(CONFIG_DIR_OVERRIDE, dir.path());

        let key = test_key();
        let account = Account {
            id: "acc-1".into(),
            name: "Personal Vaultwarden".into(),
            server_url: "https://vw.example.com".into(),
            email: "user@example.com".into(),
            access_token: None,
            refresh_token: None,
            protected_key: None,
            protected_private_key: None,
            last_sync_timestamp: 0,
            auto_lock_minutes: 15,
            device_id: "dev-uuid-1".into(),
            kdf_type: Some(0),
            kdf_iterations: Some(600_000),
            kdf_memory: None,
            kdf_parallelism: None,
        };

        add_account(&key, account.clone()).unwrap();
        let loaded = load(&key).unwrap();
        assert_eq!(loaded.accounts.len(), 1);
        assert_eq!(loaded.accounts[0].name, "Personal Vaultwarden");

        set_active_account(&key, "acc-1").unwrap();
        let loaded = load(&key).unwrap();
        assert_eq!(loaded.active_account_id.as_deref(), Some("acc-1"));

        remove_account(&key, "acc-1").unwrap();
        let loaded = load(&key).unwrap();
        assert!(loaded.accounts.is_empty());

        std::env::remove_var(CONFIG_DIR_OVERRIDE);
    }
}
