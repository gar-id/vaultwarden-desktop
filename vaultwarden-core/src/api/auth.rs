use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::api::error::ApiError;
use crate::crypto::{KdfParams, KdfType};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KdfSettingsObj {
    #[serde(alias = "kdfType", alias = "KdfType", alias = "kdf", alias = "Kdf")]
    pub kdf_type: Option<u32>,
    #[serde(alias = "iterations", alias = "Iterations", alias = "kdfIterations", alias = "KdfIterations")]
    pub iterations: Option<u32>,
    #[serde(alias = "memory", alias = "Memory", alias = "kdfMemory", alias = "KdfMemory")]
    pub memory: Option<u32>,
    #[serde(alias = "parallelism", alias = "Parallelism", alias = "kdfParallelism", alias = "KdfParallelism")]
    pub parallelism: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PreloginResponse {
    #[serde(alias = "kdf", alias = "Kdf", alias = "kdfType", alias = "KdfType")]
    pub kdf: Option<u32>,
    #[serde(alias = "kdfIterations", alias = "KdfIterations", alias = "iterations", alias = "Iterations")]
    pub kdf_iterations: Option<u32>,
    #[serde(alias = "kdfMemory", alias = "KdfMemory", alias = "memory", alias = "Memory")]
    pub kdf_memory: Option<u32>,
    #[serde(alias = "kdfParallelism", alias = "KdfParallelism", alias = "parallelism", alias = "Parallelism")]
    pub kdf_parallelism: Option<u32>,
    #[serde(alias = "kdfSettings", alias = "KdfSettings")]
    pub kdf_settings: Option<KdfSettingsObj>,
}

impl PreloginResponse {
    pub fn to_kdf_params(&self) -> KdfParams {
        let kdf_type_num = self.kdf
            .or_else(|| self.kdf_settings.as_ref().and_then(|s| s.kdf_type))
            .unwrap_or(0);

        let kdf_type = match kdf_type_num {
            1 => KdfType::Argon2id,
            _ => KdfType::Pbkdf2,
        };

        let iterations = self.kdf_iterations
            .or_else(|| self.kdf_settings.as_ref().and_then(|s| s.iterations));

        let memory = self.kdf_memory
            .or_else(|| self.kdf_settings.as_ref().and_then(|s| s.memory));

        let parallelism = self.kdf_parallelism
            .or_else(|| self.kdf_settings.as_ref().and_then(|s| s.parallelism));

        match kdf_type {
            KdfType::Pbkdf2 => {
                KdfParams::pbkdf2(iterations.unwrap_or(600_000))
            }
            KdfType::Argon2id => {
                KdfParams::argon2id(
                    iterations.unwrap_or(3),
                    memory.unwrap_or(64),
                    parallelism.unwrap_or(4),
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
    #[serde(alias = "Key", alias = "key")]
    pub key: Option<String>,
    #[serde(alias = "PrivateKey", alias = "privateKey")]
    pub private_key: Option<String>,
    #[serde(alias = "Kdf", alias = "kdf", alias = "kdfType")]
    pub kdf: Option<u32>,
    #[serde(alias = "KdfIterations", alias = "kdfIterations", alias = "iterations")]
    pub kdf_iterations: Option<u32>,
    #[serde(alias = "KdfMemory", alias = "kdfMemory", alias = "memory")]
    pub kdf_memory: Option<u32>,
    #[serde(alias = "KdfParallelism", alias = "kdfParallelism", alias = "parallelism")]
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

    let device_type = if cfg!(target_os = "windows") {
        "6"
    } else if cfg!(target_os = "macos") {
        "7"
    } else {
        "8"
    };

    let device_name = if cfg!(target_os = "windows") {
        "Windows Desktop"
    } else if cfg!(target_os = "macos") {
        "macOS Desktop"
    } else {
        "Linux Desktop"
    };

    let mut form = HashMap::new();
    form.insert("grant_type", "password".to_string());
    form.insert("username", email.trim().to_lowercase());
    form.insert("password", master_password_hash.to_string());
    form.insert("scope", "api offline_access".to_string());
    form.insert("client_id", "desktop".to_string());
    form.insert("deviceType", device_type.to_string());
    form.insert("deviceIdentifier", device_id.to_string());
    form.insert("deviceName", device_name.to_string());

        if let Some(ref tf) = two_factor {
            form.insert("twoFactorProvider", tf.provider.to_string());
            form.insert("twoFactorTokenProvider", tf.provider.to_string());
            form.insert("twoFactorToken", tf.token.trim().to_string());
            if tf.remember {
                form.insert("twoFactorRemember", "1".to_string());
            }
        }

        let resp = client.post(&url).form(&form).send().await?;

        if resp.status() == 400 {
            let err_json: serde_json::Value = resp.json().await.unwrap_or_default();

            // Only trigger TwoFactorRequired if user did not supply a 2FA token
            if two_factor.is_none() {
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
            } else {
                // If two_factor was provided, check if it was invalid
                let desc = err_json
                    .get("ErrorModel")
                    .and_then(|m| m.get("Message"))
                    .and_then(|v| v.as_str())
                    .or_else(|| err_json.get("error_description").and_then(|v| v.as_str()))
                    .unwrap_or("");
                if desc.contains("Two-factor") || desc.contains("twoFactor") || desc.contains("invalid_token") || desc == "invalid_grant" {
                    return Err(ApiError::Auth(
                        "Invalid two-step verification code. Please check your authenticator and try again.".into()
                    ));
                }
            }

            let desc = err_json
                .get("ErrorModel")
                .and_then(|m| m.get("Message"))
                .and_then(|v| v.as_str())
                .or_else(|| err_json.get("error_description").and_then(|v| v.as_str()))
                .or_else(|| err_json.get("message").and_then(|v| v.as_str()))
                .unwrap_or("Invalid email or master password");

            let friendly_desc = if desc == "invalid_username_or_password" {
                "Username or password is incorrect. Please check your email and master password."
            } else {
                desc
            };

            return Err(ApiError::Auth(friendly_desc.to_string()));
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

/// Triggers email 2FA verification code dispatch on the server.
pub async fn send_two_factor_email(
    client: &Client,
    api_url: &str,
    email: &str,
    master_password_hash: &str,
) -> Result<(), ApiError> {
    let url = format!("{}/two-factor/send-email-login", api_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "email": email.trim().to_lowercase(),
        "masterPasswordHash": master_password_hash
    });
    let resp = client.post(&url).json(&body).send().await?;
    if !resp.status().is_success() {
        log::warn!("send_two_factor_email returned status {}", resp.status());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prelogin_response_us_and_eu() {
        let us_json = r#"{"kdf":0,"kdfIterations":600000,"kdfMemory":null,"kdfParallelism":null}"#;
        let us: PreloginResponse = serde_json::from_str(us_json).unwrap();
        assert_eq!(us.to_kdf_params(), KdfParams::pbkdf2(600_000));

        let eu_json = r#"{"kdf":1,"kdfIterations":6,"kdfMemory":32,"kdfParallelism":4,"kdfSettings":{"kdfType":1,"iterations":6,"memory":32,"parallelism":4},"salt":null}"#;
        let eu: PreloginResponse = serde_json::from_str(eu_json).unwrap();
        assert_eq!(eu.to_kdf_params(), KdfParams::argon2id(6, 32, 4));

        let pascal_json = r#"{"Kdf":0,"KdfIterations":100000,"KdfMemory":null,"KdfParallelism":null}"#;
        let pascal: PreloginResponse = serde_json::from_str(pascal_json).unwrap();
        assert_eq!(pascal.to_kdf_params(), KdfParams::pbkdf2(100_000));
    }
}
