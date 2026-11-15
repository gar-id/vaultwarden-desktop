use reqwest::Client;
use serde_json::json;

use crate::api::error::ApiError;
use crate::api::sync::SyncFolder;

/// Create a new folder on the server (POST /folders)
pub async fn create_folder(
    client: &Client,
    api_url: &str,
    access_token: &str,
    enc_name: &str,
) -> Result<SyncFolder, ApiError> {
    let url = format!("{}/folders", api_url.trim_end_matches('/'));
    let req = json!({ "name": enc_name });
    let resp = client
        .post(&url)
        .bearer_auth(access_token)
        .json(&req)
        .send()
        .await?;

    if resp.status() == 401 {
        return Err(ApiError::SessionExpired);
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(ApiError::Auth(format!("Create folder failed ({status}): {body}")));
    }

    let folder: SyncFolder = resp.json().await?;
    Ok(folder)
}

/// Update an existing folder on the server (PUT /folders/{id})
pub async fn update_folder(
    client: &Client,
    api_url: &str,
    access_token: &str,
    folder_id: &str,
    enc_name: &str,
) -> Result<SyncFolder, ApiError> {
    let url = format!("{}/folders/{}", api_url.trim_end_matches('/'), folder_id);
    let req = json!({ "name": enc_name });
    let resp = client
        .put(&url)
        .bearer_auth(access_token)
        .json(&req)
        .send()
        .await?;

    if resp.status() == 401 {
        return Err(ApiError::SessionExpired);
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(ApiError::Auth(format!("Update folder failed ({status}): {body}")));
    }

    let folder: SyncFolder = resp.json().await?;
    Ok(folder)
}

/// Delete a folder on the server (DELETE /folders/{id})
pub async fn delete_folder(
    client: &Client,
    api_url: &str,
    access_token: &str,
    folder_id: &str,
) -> Result<(), ApiError> {
    let url = format!("{}/folders/{}", api_url.trim_end_matches('/'), folder_id);
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
        return Err(ApiError::Auth(format!("Delete folder failed ({status}): {body}")));
    }

    Ok(())
}
