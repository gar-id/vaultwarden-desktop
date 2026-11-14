use rand::RngCore;
use serde_json::json;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::get_ipc_response;
use tauri::webview::InvokeRequest;
use tauri::WebviewWindowBuilder;
use vaultwarden_core::crypto::{self, KdfParams, SymmetricKey};
use vaultwarden_core::storage::accounts::{self, Account};
use vaultwarden_core::storage::db;

static CONFIG_DIR_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn invoke(
    webview: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    cmd: &str,
    payload: serde_json::Value,
) -> Result<serde_json::Value, serde_json::Value> {
    let body = InvokeBody::Json(payload);
    get_ipc_response(
        webview,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "http://localhost:5173".parse().unwrap(),
            body,
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        },
    )
    .map(|b| b.deserialize::<serde_json::Value>().unwrap())
}

#[test]
fn full_vaultwarden_flow_add_switch_unlock_search_reveal_lock() {
    let _guard = CONFIG_DIR_ENV_LOCK.lock().unwrap();
    let config_dir = tempfile::tempdir().expect("create temp config dir");
    std::env::set_var("VAULTWARDEN_DESKTOP_CONFIG_DIR", config_dir.path());

    let mut master_key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut master_key);

    let builder = tauri::test::mock_builder();
    let app = vaultwarden_desktop_lib::build_app(builder, master_key)
        .build(tauri::generate_context!(
            assets = tauri::test::noop_assets(),
            test = true
        ))
        .expect("failed to build mock app");
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("failed to build mock webview");

    // 1. Fresh app has no profiles
    let profiles_list = invoke(&webview, "list_profiles", json!({})).expect("list_profiles ok");
    assert_eq!(profiles_list.as_array().unwrap().len(), 0);

    // 2. Prepare mock encryption keys
    let test_email = "test@example.com";
    let test_password = "master-password-123";
    let kdf = KdfParams::pbkdf2(600_000);

    let m_key = crypto::derive_master_key(test_password, test_email, &kdf).unwrap();
    let stretched = crypto::stretch_master_key(&m_key).unwrap();

    let user_symm_key = SymmetricKey::generate();
    let mut enc_k = [0u8; 32];
    let mut mac_k = [0u8; 32];
    enc_k.copy_from_slice(&stretched[0..32]);
    mac_k.copy_from_slice(&stretched[32..64]);

    let protected_key = crypto::encrypt_bytes(&user_symm_key.to_bytes(), &enc_k, &mac_k).unwrap();

    let account_id = uuid::Uuid::new_v4().to_string();
    let account = Account {
        id: account_id.clone(),
        name: "Personal Vaultwarden".into(),
        server_url: "https://vw.example.com".into(),
        email: test_email.into(),
        access_token: None,
        refresh_token: None,
        protected_key: Some(protected_key),
        protected_private_key: None,
        last_sync_timestamp: 0,
        auto_lock_minutes: 15,
        device_id: "dev-1".into(),
        kdf_type: Some(0),
        kdf_iterations: Some(600_000),
        kdf_memory: None,
        kdf_parallelism: None,
    };

    accounts::add_account(&master_key, account).expect("seed account");

    // 3. Check list_profiles shows the new account
    let profiles_list = invoke(&webview, "list_profiles", json!({})).expect("list_profiles ok");
    assert_eq!(profiles_list.as_array().unwrap().len(), 1);

    // 4. Switch to account
    let switch_res = invoke(&webview, "switch_profile", json!({ "profileId": account_id }))
        .expect("switch_profile ok");
    assert_eq!(switch_res, "locked");

    // 5. Wrong password rejected
    let err = invoke(&webview, "unlock_vault", json!({ "passphrase": "wrong-password" }))
        .expect_err("wrong password must be rejected");
    assert!(err.as_str().unwrap().contains("Invalid master password"));

    // 6. Correct password unlocks vault
    let unlocked = invoke(&webview, "unlock_vault", json!({ "passphrase": test_password }))
        .expect("correct password unlocks");
    assert_eq!(unlocked["success"], true);

    let state = invoke(&webview, "get_vault_state", json!({})).expect("get_vault_state ok");
    assert_eq!(state["locked"], false);

    // 7. Seed one cipher into the SQLite database
    let db_path = accounts::account_db_dir(&account_id).join("vault.db");
    {
        let conn = db::open(&db_path).expect("open db");
        let name_enc = user_symm_key.encrypt_string("GitHub Account").unwrap();
        let pass_enc = user_symm_key.encrypt_string("my-super-secret").unwrap();

        let login = vaultwarden_core::api::SyncCipherLogin {
            username: Some(user_symm_key.encrypt_string("tegar").unwrap()),
            password: Some(pass_enc),
            totp: None,
            uris: None,
        };

        let cipher = db::CipherItem {
            id: "cipher-1".into(),
            organization_id: None,
            folder_id: None,
            cipher_type: 1,
            name_enc,
            notes_enc: Some(user_symm_key.encrypt_string("Some notes").unwrap()),
            login_enc: Some(serde_json::to_string(&login).unwrap()),
            card_enc: None,
            identity_enc: None,
            fields_enc: None,
            key_enc: None,
            favorite: false,
            revision_date: 1700000000,
            deleted_date: None,
        };

        db::upsert_cipher(
            &conn,
            &cipher,
            "GitHub Account",
            Some("tegar"),
            Some("https://github.com"),
        )
        .expect("upsert cipher");
    }

    // 8. Search resources
    let results = invoke(&webview, "search_resources", json!({ "query": "GitHub" }))
        .expect("search_resources ok");
    let arr = results.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], "cipher-1");

    // 9. Reveal secret
    let revealed = invoke(&webview, "reveal_secret", json!({ "resourceId": "cipher-1" }))
        .expect("reveal_secret ok");
    assert_eq!(revealed["password"], "my-super-secret");
    assert_eq!(revealed["description"], "Some notes");

    // 10. Lock vault
    invoke(&webview, "lock_vault", json!({})).expect("lock ok");
    let state = invoke(&webview, "get_vault_state", json!({})).expect("get_vault_state ok");
    assert_eq!(state["locked"], true);

    let err = invoke(&webview, "reveal_secret", json!({ "resourceId": "cipher-1" }))
        .expect_err("reveal after lock must fail");
    assert_eq!(err, "Vault is locked");

    std::env::remove_var("VAULTWARDEN_DESKTOP_CONFIG_DIR");
}

#[test]
fn generate_password_respects_rules() {
    let _guard = CONFIG_DIR_ENV_LOCK.lock().unwrap();
    let config_dir = tempfile::tempdir().expect("create temp config dir");
    std::env::set_var("VAULTWARDEN_DESKTOP_CONFIG_DIR", config_dir.path());

    let mut master_key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut master_key);

    let builder = tauri::test::mock_builder();
    let app = vaultwarden_desktop_lib::build_app(builder, master_key)
        .build(tauri::generate_context!(
            assets = tauri::test::noop_assets(),
            test = true
        ))
        .expect("failed to build mock app");
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("failed to build mock webview");

    let password = invoke(
        &webview,
        "generate_password",
        json!({
            "rules": {
                "length": 24,
                "uppercase": true,
                "lowercase": true,
                "numbers": true,
                "symbols": false,
            }
        }),
    )
    .expect("generate_password ok");
    let password = password.as_str().unwrap();

    assert_eq!(password.len(), 24);
    assert!(password.chars().all(|c| c.is_ascii_alphanumeric()));

    std::env::remove_var("VAULTWARDEN_DESKTOP_CONFIG_DIR");
}
