use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::api::error::ApiError;
use crate::crypto::{KdfParams, KdfType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreloginResponse {
    #[serde(rename = "Kdf")]
    pub kdf: Option<u32>,
    #[serde(rename = "KdfIterations")]
    pub kdf_iterations: Option<u32>,
    #[serde(rename = "KdfMemory")]
    pub kdf_memory: Option<u32>,
    #[serde(rename = "KdfParallelism")]
    pub kdf_parallelism: Option<u32>,
}

impl PreloginResponse {
    pub fn to_kdf_params(&self) -> KdfParams {
        let kdf_type = match self.kdf.unwrap_or(0) {
            1 => KdfType::Argon2id,
            _ => KdfType::Pbkdf2,
        };

        match kdf_type {
            KdfType::Pbkdf2 => {
                KdfParams::pbkdf2(self.kdf_iterations.unwrap_or(600_000))
            }
            KdfType::Argon2id => {
                KdfParams::argon2id(
                    self.kdf_iterations.unwrap_or(3),
                    self.kdf_memory.unwrap_or(64),
                    self.kdf_parallelism.unwrap_or(4),
                )
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: i64,
    pub token_type: String,
    #[serde(rename = "Key")]
    pub key: Option<String>,
    #[serde(rename = "PrivateKey")]
    pub private_key: Option<String>,
    #[serde(rename = "Kdf")]
    pub kdf: Option<u32>,
    #[serde(rename = "KdfIterations")]
    pub kdf_iterations: Option<u32>,
    #[serde(rename = "KdfMemory")]
    pub kdf_memory: Option<u32>,
    #[serde(rename = "KdfParallelism")]
    pub kdf_parallelism: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct TwoFactorInput {
    pub provider: i32, // 0 = Authenticator app (TOTP), 1 = Email, 3 = YubiKey, etc.
    pub token: String,
    pub remember: bool,
}

/// Calls POST /accounts/prelogin to fetch the user's KDF configuration.
pub async fn prelogin(
    client: &Client,
    identity_url: &str,
    email: &str,
) -> Result<PreloginResponse, ApiError> {
    let url = format!("{}/accounts/prelogin", identity_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "email": email.trim().to_lowercase()
    });

    let resp = client.post(&url).json(&body).send().await?;
    if !resp.status().is_success() {
        return Err(ApiError::Auth(format!(
            "Prelogin failed with status {}",
            resp.status()
        )));
    }

    let prelogin_res: PreloginResponse = resp.json().await?;
    Ok(prelogin_res)
}

/// Calls POST /connect/token to log in via OAuth2 password grant.
pub async fn login(
    client: &Client,
    identity_url: &str,
    email: &str,
    master_password_hash: &str,
    device_id: &str,
    two_factor: Option<TwoFactorInput>,
) -> Result<TokenResponse, ApiError> {
    let url = format!("{}/connect/token", identity_url.trim_end_matches('/'));

    let mut form = HashMap::new();
    form.insert("grant_type", "password".to_string());
    form.insert("username", email.trim().to_lowercase());
    form.insert("password", master_password_hash.to_string());
    form.insert("scope", "api offline_access".to_string());
    form.insert("client_id", "desktop".to_string());
    form.insert("deviceType", "8".to_string()); // LinuxDesktop
    form.insert("deviceIdentifier", device_id.to_string());
    form.insert("deviceName", "Vaultwarden Desktop".to_string());

    if let Some(tf) = two_factor {
        form.insert("twoFactorProvider", tf.provider.to_string());
        form.insert("twoFactorToken", tf.token);
        if tf.remember {
            form.insert("twoFactorRemember", "1".to_string());
        }
    }

    let resp = client.post(&url).form(&form).send().await?;

    if resp.status() == 400 {
        let err_json: serde_json::Value = resp.json().await.unwrap_or_default();
        if let Some(providers_val) = err_json.get("TwoFactorProviders2")
            .or_else(|| err_json.get("TwoFactorProviders"))
        {
            let providers: Vec<i32> = if let Some(obj) = providers_val.as_object() {
                obj.keys().filter_map(|k| k.parse().ok()).collect()
            } else if let Some(arr) = providers_val.as_array() {
                arr.iter().filter_map(|v| v.as_i64().map(|i| i as i32)).collect()
            } else {
                vec![]
            };

            let auth_token = err_json
                .get("auth_token")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            return Err(ApiError::TwoFactorRequired {
                providers,
                auth_token,
            });
        }

        let desc = err_json
            .get("error_description")
            .and_then(|v| v.as_str())
            .unwrap_or("Invalid email or master password");
        return Err(ApiError::Auth(desc.to_string()));
    }

    if !resp.status().is_success() {
        return Err(ApiError::Auth(format!("Login failed: {}", resp.status())));
    }

    let token_res: TokenResponse = resp.json().await?;
    Ok(token_res)
}

/// Calls POST /connect/token to refresh an expired access token using a refresh token.
pub async fn refresh_access_token(
    client: &Client,
    identity_url: &str,
    refresh_token: &str,
) -> Result<TokenResponse, ApiError> {
    let url = format!("{}/connect/token", identity_url.trim_end_matches('/'));

    let mut form = HashMap::new();
    form.insert("grant_type", "refresh_token");
    form.insert("refresh_token", refresh_token);
    form.insert("client_id", "desktop");

    let resp = client.post(&url).form(&form).send().await?;
    if !resp.status().is_success() {
        return Err(ApiError::SessionExpired);
    }

    let token_res: TokenResponse = resp.json().await?;
    Ok(token_res)
}
