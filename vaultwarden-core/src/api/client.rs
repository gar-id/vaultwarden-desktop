use reqwest::{Client, Url, cookie::{CookieStore, Jar}};
use std::sync::Arc;

/// An HTTP client coupled with its persistent cookie jar.
/// Exposes helpers to read cookies (such as `csrfToken`) back out of the jar
/// at any time, eliminating fragile header-catching on individual responses.
#[derive(Clone, Debug)]
pub struct HttpClient {
    pub client: Client,
    pub jar: Arc<Jar>,
}

impl HttpClient {
    pub fn new() -> Self {
        let jar = Arc::new(Jar::default());
        let client = Client::builder()
            .cookie_provider(jar.clone())
            .use_rustls_tls()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");
        Self { client, jar }
    }

    /// Extract the current `csrfToken` cookie value from the jar for a given server URL.
    pub fn get_csrf_token(&self, server_url: &str) -> Option<String> {
        let url = Url::parse(server_url).ok()?;
        let header = self.jar.cookies(&url)?;
        let header_str = header.to_str().ok()?;
        for part in header_str.split(';') {
            let part = part.trim();
            if let Some(val) = part.strip_prefix("csrfToken=") {
                return Some(val.to_string());
            }
        }
        None
    }

    /// Ensure a valid CSRF token is available. If already in the jar, returns
    /// immediately in 0ms; if missing, issues a lightweight GET to `/auth/verify.json`
    /// which causes the server to set the cookie into the jar.
    pub async fn ensure_csrf_token(&self, server_url: &str) -> Result<String, String> {
        if let Some(token) = self.get_csrf_token(server_url) {
            return Ok(token);
        }
        let url = format!("{}/auth/verify.json", server_url.trim_end_matches('/'));
        let _ = self.client.get(&url).send().await;
        self.get_csrf_token(server_url)
            .ok_or_else(|| "Could not obtain CSRF token from server".to_string())
    }
}

impl std::ops::Deref for HttpClient {
    type Target = Client;
    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

/// Build a reqwest Client with persistent cookie jar and TLS via rustls.
pub fn build_client() -> HttpClient {
    HttpClient::new()
}
