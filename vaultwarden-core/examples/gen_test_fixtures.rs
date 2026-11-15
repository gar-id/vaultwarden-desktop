use vaultwarden_core::api::ServerEndpoints;
use vaultwarden_core::crypto;
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    let email = "rizkygalrd@gmail.com";
    let password = "1kyC99eTOv/WB69uN1L8J2DNLnqDFEdEiL5/";

    let client = reqwest::Client::new();
    let endpoints = ServerEndpoints::bitwarden_eu();

    // 1. Prelogin
    let pre_url = format!("{}/accounts/prelogin", endpoints.identity_url);
    let pre_res = client.post(&pre_url)
        .json(&serde_json::json!({"email": email}))
        .send().await.unwrap()
        .text().await.unwrap();
    println!("Prelogin EU: {}", pre_res);

    let kdf_params = crypto::KdfParams::pbkdf2(100_000);
    let master_key = crypto::derive_master_key(password, email, &kdf_params).unwrap();
    let hash = crypto::derive_master_password_hash(&master_key, password).unwrap();

    let client_ids = vec![
        ("desktop", "8", "Linux Desktop"),
        ("web", "9", "chrome"),
        ("browser", "2", "chrome"),
        ("cli", "25", "Linux CLI"),
    ];

    let token_url = format!("{}/connect/token", endpoints.identity_url);

    for (cid, dtype, dname) in client_ids {
        let mut form = HashMap::new();
        form.insert("grant_type", "password");
        form.insert("username", email);
        form.insert("password", &hash);
        form.insert("scope", "api offline_access");
        form.insert("client_id", cid);
        form.insert("deviceType", dtype);
        form.insert("deviceIdentifier", "7d44c82b-65e1-45da-9ae0-4a88f72c11e3");
        form.insert("deviceName", dname);

        let resp = client.post(&token_url).form(&form).send().await.unwrap();
        let status = resp.status();
        let body = resp.text().await.unwrap();
        println!("[{cid}] status={status}: {body}");
    }
}
