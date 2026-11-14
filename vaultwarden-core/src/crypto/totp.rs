use std::time::SystemTime;
use totp_rs::{Algorithm, Builder, Secret, Totp};
use super::error::CryptoError;

#[derive(Debug, Clone)]
pub struct TotpCodeResult {
    pub code: String,
    pub expires_in_secs: u32,
}

/// Generates a live TOTP 6-digit code and remaining seconds from either an
/// `otpauth://` URI or a raw base32 secret.
pub fn generate_totp_code(secret_or_url: &str) -> Result<TotpCodeResult, CryptoError> {
    let trimmed = secret_or_url.trim();

    let totp = if trimmed.starts_with("otpauth://") {
        Totp::from_url_unchecked(trimmed)
            .map_err(|e| CryptoError::Totp(format!("Invalid TOTP url: {e:?}")))?
    } else {
        let sanitized: String = trimmed.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
        let secret = Secret::try_from_base32(&sanitized)
            .map_err(|e| CryptoError::Totp(format!("Invalid base32 secret: {e:?}")))?;

        Builder::new()
            .with_algorithm(Algorithm::SHA1)
            .with_digits(6)
            .with_step_duration(30)
            .with_secret(secret.as_bytes().to_vec())
            .build_noncompliant()
    };

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| CryptoError::Totp(e.to_string()))?
        .as_secs();

    let code = totp.generate(now);
    let step = totp.step();
    let expires_in_secs = (step - (now % step)) as u32;

    Ok(TotpCodeResult {
        code: code.to_string(),
        expires_in_secs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_totp_generation_from_base32() {
        let res = generate_totp_code("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ").unwrap();
        assert_eq!(res.code.len(), 6);
        assert!(res.expires_in_secs <= 30);
    }

    #[test]
    fn test_totp_generation_from_url() {
        let url = "otpauth://totp/Example:alice@google.com?secret=JBSWY3DPEHPK3PXP&issuer=Example";
        let res = generate_totp_code(url).unwrap();
        assert_eq!(res.code.len(), 6);
        assert!(res.expires_in_secs <= 30);
    }
}
