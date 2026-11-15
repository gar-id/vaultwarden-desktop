use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::api::error::ApiError;
use crate::api::sync::{
    SyncCipher, SyncCipherCard, SyncCipherField, SyncCipherIdentity, SyncCipherLogin,
    SyncPasswordHistory,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CipherRequest {
    #[serde(rename = "type")]
    pub cipher_type: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub favorite: bool,
    pub reprompt: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_for: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login: Option<SyncCipherLogin>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<SyncCipherCard>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<SyncCipherIdentity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<SyncCipherField>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_history: Option<Vec<SyncPasswordHistory>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// Create a new cipher on the server (POST /ciphers)
pub async fn create_cipher(
    client: &Client,
    api_url: &str,
    access_token: &str,
    request: &CipherRequest,
) -> Result<SyncCipher, ApiError> {
    let url = format!("{}/ciphers", api_url.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .bearer_auth(access_token)
        .json(request)
        .send()
        .await?;

    if resp.status() == 401 {
        return Err(ApiError::SessionExpired);
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(ApiError::Auth(format!("Create cipher failed ({status}): {body}")));
    }

    let cipher: SyncCipher = resp.json().await?;
    Ok(cipher)
}

/// Update an existing cipher on the server (PUT /ciphers/{id})
pub async fn update_cipher(
    client: &Client,
    api_url: &str,
    access_token: &str,
    cipher_id: &str,
    request: &CipherRequest,
) -> Result<SyncCipher, ApiError> {
    let url = format!("{}/ciphers/{}", api_url.trim_end_matches('/'), cipher_id);
    let resp = client
        .put(&url)
        .bearer_auth(access_token)
        .json(request)
        .send()
        .await?;

    if resp.status() == 401 {
        return Err(ApiError::SessionExpired);
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(ApiError::Auth(format!("Update cipher failed ({status}): {body}")));
    }

    let cipher: SyncCipher = resp.json().await?;
    Ok(cipher)
}

/// Delete a cipher on the server (DELETE /ciphers/{id})
pub async fn delete_cipher(
    client: &Client,
    api_url: &str,
    access_token: &str,
    cipher_id: &str,
) -> Result<(), ApiError> {
    let url = format!("{}/ciphers/{}", api_url.trim_end_matches('/'), cipher_id);
    let resp = client
        .delete(&url)
        .bearer_auth(access_token)
        .send()
        .await?;

    if resp.status() == 401 {
        return Err(ApiError::SessionExpired);
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(ApiError::Auth(format!("Delete cipher failed ({status}): {body}")));
    }

    Ok(())
}
