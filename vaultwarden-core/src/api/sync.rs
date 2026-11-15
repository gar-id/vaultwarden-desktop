use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::api::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProfileOrganization {
    pub id: String,
    pub name: String,
    pub key: Option<String>,
    pub status: Option<i32>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProfile {
    pub id: String,
    pub name: Option<String>,
    pub email: String,
    pub key: Option<String>,
    #[serde(rename = "privateKey")]
    pub private_key: Option<String>,
    pub organizations: Option<Vec<SyncProfileOrganization>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCipherUri {
    pub uri: Option<String>,
    pub match_type: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCipherLogin {
    pub username: Option<String>,
    pub password: Option<String>,
    pub totp: Option<String>,
    pub uris: Option<Vec<SyncCipherUri>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCipherCard {
    #[serde(rename = "cardholderName")]
    pub cardholder_name: Option<String>,
    pub brand: Option<String>,
    pub number: Option<String>,
    #[serde(rename = "expMonth")]
    pub exp_month: Option<String>,
    #[serde(rename = "expYear")]
    pub exp_year: Option<String>,
    pub code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCipherIdentity {
    pub title: Option<String>,
    #[serde(rename = "firstName")]
    pub first_name: Option<String>,
    #[serde(rename = "middleName")]
    pub middle_name: Option<String>,
    #[serde(rename = "lastName")]
    pub last_name: Option<String>,
    pub address1: Option<String>,
    pub address2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    #[serde(rename = "postalCode")]
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub company: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub ssn: Option<String>,
    pub username: Option<String>,
    #[serde(rename = "passportNumber")]
    pub passport_number: Option<String>,
    #[serde(rename = "licenseNumber")]
    pub license_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCipherField {
    pub name: Option<String>,
    pub value: Option<String>,
    #[serde(rename = "type")]
    pub field_type: Option<i32>, // 0=Text, 1=Hidden, 2=Boolean, 3=Linked
    #[serde(rename = "linkedId")]
    pub linked_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPasswordHistory {
    #[serde(rename = "lastUsedDate")]
    pub last_used_date: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCipherAttachment {
    pub id: String,
    pub url: Option<String>,
    #[serde(rename = "fileName")]
    pub file_name: Option<String>,
    pub size: Option<String>,
    #[serde(rename = "sizeName")]
    pub size_name: Option<String>,
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCipher {
    pub id: String,
    #[serde(rename = "organizationId")]
    pub organization_id: Option<String>,
    #[serde(rename = "folderId")]
    pub folder_id: Option<String>,
    #[serde(rename = "type")]
    pub cipher_type: i32, // 1=Login, 2=SecureNote, 3=Card, 4=Identity
    pub name: String,
    pub notes: Option<String>,
    pub key: Option<String>,
    pub favorite: Option<bool>,
    pub login: Option<SyncCipherLogin>,
    pub card: Option<SyncCipherCard>,
    pub identity: Option<SyncCipherIdentity>,
    pub fields: Option<Vec<SyncCipherField>>,
    pub attachments: Option<Vec<SyncCipherAttachment>>,
    #[serde(rename = "passwordHistory")]
    pub password_history: Option<Vec<SyncPasswordHistory>>,
    #[serde(rename = "collectionIds")]
    pub collection_ids: Option<Vec<String>>,
    #[serde(rename = "revisionDate")]
    pub revision_date: Option<String>,
    #[serde(rename = "deletedDate")]
    pub deleted_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncFolder {
    pub id: String,
    pub name: String,
    #[serde(rename = "revisionDate")]
    pub revision_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCollection {
    pub id: String,
    #[serde(rename = "organizationId")]
    pub organization_id: String,
    pub name: String,
    #[serde(rename = "readOnly")]
    pub read_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResponse {
    pub profile: SyncProfile,
    pub ciphers: Option<Vec<SyncCipher>>,
    pub folders: Option<Vec<SyncFolder>>,
    pub collections: Option<Vec<SyncCollection>>,
}

/// Calls GET /sync to download the entire vault dataset.
pub async fn fetch_sync_data(
    client: &Client,
    api_url: &str,
    access_token: &str,
) -> Result<SyncResponse, ApiError> {
    let url = format!("{}/sync?excludeDomains=true", api_url.trim_end_matches('/'));

    let resp = client
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .await?;

    if resp.status() == 401 {
        return Err(ApiError::SessionExpired);
    }
    if !resp.status().is_success() {
        return Err(ApiError::Http(resp.error_for_status().unwrap_err()));
    }

    let sync_data: SyncResponse = resp.json().await?;
    Ok(sync_data)
}
