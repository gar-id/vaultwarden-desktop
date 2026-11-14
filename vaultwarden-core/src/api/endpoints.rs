use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerEndpoints {
    pub base_url: String,
    pub identity_url: String,
    pub api_url: String,
}

impl ServerEndpoints {
    pub fn bitwarden_us() -> Self {
        Self {
            base_url: "https://vault.bitwarden.com".into(),
            identity_url: "https://identity.bitwarden.com".into(),
            api_url: "https://api.bitwarden.com".into(),
        }
    }

    pub fn bitwarden_eu() -> Self {
        Self {
            base_url: "https://vault.bitwarden.eu".into(),
            identity_url: "https://identity.bitwarden.eu".into(),
            api_url: "https://api.bitwarden.eu".into(),
        }
    }

    /// Auto-detects endpoints from a given server URL.
    /// If user enters "https://vault.bitwarden.com", routes to US cloud.
    /// If user enters "https://vault.bitwarden.eu", routes to EU cloud.
    /// For self-hosted (Vaultwarden or official server), uses `<url>/identity` and `<url>/api`.
    pub fn from_base_url(base_url: &str) -> Self {
        let trimmed = base_url.trim().trim_end_matches('/');

        if trimmed == "https://vault.bitwarden.com" || trimmed == "https://bitwarden.com" {
            return Self::bitwarden_us();
        }
        if trimmed == "https://vault.bitwarden.eu" || trimmed == "https://bitwarden.eu" {
            return Self::bitwarden_eu();
        }

        Self {
            base_url: trimmed.to_string(),
            identity_url: format!("{trimmed}/identity"),
            api_url: format!("{trimmed}/api"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoints() {
        let us = ServerEndpoints::from_base_url("https://vault.bitwarden.com");
        assert_eq!(us.identity_url, "https://identity.bitwarden.com");

        let self_hosted = ServerEndpoints::from_base_url("https://vw.example.com/");
        assert_eq!(self_hosted.identity_url, "https://vw.example.com/identity");
        assert_eq!(self_hosted.api_url, "https://vw.example.com/api");
    }
}
