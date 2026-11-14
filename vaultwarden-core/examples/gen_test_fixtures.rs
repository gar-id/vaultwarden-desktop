use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use vaultwarden_core::crypto::{derive_master_key, derive_master_password_hash, KdfParams};

fn main() {
    let params = KdfParams::pbkdf2(100_000);
    let master_key = derive_master_key("password123", "user@example.com", &params).unwrap();
    println!("rust master_key b64: {}", BASE64.encode(master_key));

    let hash = derive_master_password_hash(&master_key, "password123").unwrap();
    println!("rust master_key_hash b64: {}", hash);
}
