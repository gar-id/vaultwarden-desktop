mod commands;
mod state;
mod tray;

use vaultwarden_core::storage::keyring;
use state::{AppState, SafeState};
use tauri::{Manager, Runtime, WindowEvent};

/// Registers all IPC commands and app state on the given builder. Shared
/// between the real desktop entry point and the e2e test harness so both
/// exercise the exact same command wiring — only the Tauri runtime differs
/// (the real WRY runtime vs. `tauri::test::MockRuntime`).
pub fn build_app<R: Runtime>(builder: tauri::Builder<R>, master_key: [u8; 32]) -> tauri::Builder<R> {
    builder
        .manage(SafeState::new(AppState::new(master_key)))
        .invoke_handler(tauri::generate_handler![
            commands::profiles::list_profiles,
            commands::profiles::add_profile,
            commands::profiles::remove_profile,
            commands::profiles::switch_profile,
            commands::profiles::discover_server,
            commands::crypto::unlock_vault,
            commands::crypto::lock_vault,
            commands::crypto::get_vault_state,
            commands::crypto::record_user_activity,
            commands::vault::sync_vault,
            commands::vault::search_resources,
            commands::vault::list_folder_tree,
            commands::vault::reveal_secret,
            commands::vault::get_totp_code,
            commands::vault::copy_secret,
            commands::vault::generate_password,
            commands::vault::create_resource,
            commands::vault::update_resource,
            commands::vault::delete_resource,
            commands::vault::create_folder,
            commands::vault::rename_folder,
            commands::vault::delete_folder,
            commands::vault::move_item,
            commands::vault::toggle_favorite,
            commands::vault::search_share_targets,
            commands::vault::get_item_permissions,
            commands::vault::share_item,
            commands::vault::list_vault_tags,
            commands::vault::update_resource_tags,
            commands::vault::get_resource_comments,
            commands::vault::add_resource_comment,
            commands::vault::delete_resource_comment,
            commands::vault::get_resource_activity,
            commands::vault::force_resync,
            commands::vault::clear_local_cache,
            commands::crypto::get_app_settings,
            commands::crypto::set_app_settings,
        ])
}

/// Builds the logging plugin. Writes to stdout (visible when launched from a
/// terminal) AND to a file in the OS app-log directory, AND forwards webview
/// `console.log`/`console.error`/unhandled JS errors into the same log via
/// `TargetKind::Webview` + the frontend's `attachConsole()` hookup in
/// `ui/src/lib/logger.ts`. This is the only way to see *why* the webview
/// cancelled a resource load (e.g. the crossorigin/CSP class of bug) since
/// that failure happens inside WebKit, below any of our own JS or Rust code.
///
/// Log file location (verified on Linux; macOS/Windows per tauri-plugin-log's
/// own platform resolution, not independently confirmed here):
///   Linux:   ~/.local/share/com.vaultwarden.desktop/logs/vaultwarden-desktop.log
///   macOS:   ~/Library/Logs/com.vaultwarden.desktop/vaultwarden-desktop.log
///   Windows: %APPDATA%\com.vaultwarden.desktop\logs\vaultwarden-desktop.log
fn build_logger() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    use tauri_plugin_log::{Target, TargetKind};

    tauri_plugin_log::Builder::new()
        .target(Target::new(TargetKind::Stdout))
        .target(Target::new(TargetKind::LogDir {
            file_name: Some("vaultwarden-desktop".into()),
        }))
        .target(Target::new(TargetKind::Webview))
        .level(log::LevelFilter::Debug)
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let master_key = keyring::get_or_create_master_key()
        .expect("Failed to access OS keyring — cannot start");

    let builder = tauri::Builder::default()
        .plugin(build_logger())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            log::info!("Second instance launched; focusing existing window and locking vault");
            lock_vault_internally(app, "app reopened via app launcher");
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Minimize to tray instead of quitting, and lock vault
                api.prevent_close();
                lock_vault_internally(window.app_handle(), "window closed to tray");
                let _ = window.hide();
            }
        });

    build_app(builder, master_key)
        .setup(|app| {
            log::info!("vaultwarden-desktop starting up");

            // Tray and global shortcut are conveniences, not core functionality.
            // A missing tray daemon or a hotkey already claimed by another app
            // (common for Ctrl+Shift+Space: screenshot tools, launchers, etc.)
            // must never crash the whole app on startup - log and continue.
            if let Err(e) = tray::build_tray(app.handle()) {
                log::warn!("system tray unavailable, continuing without it: {e}");
            }
            if let Err(e) = setup_global_shortcut(app) {
                log::warn!("global shortcut Ctrl+Shift+Space unavailable, continuing without it: {e}");
            }
            setup_auto_lock(app);

            if let Some(window) = app.get_webview_window("main") {
                if let Ok(icon) = tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png")) {
                    let _ = window.set_icon(icon);
                }
                #[cfg(debug_assertions)]
                window.open_devtools();
            }

            log::info!("setup complete");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_global_shortcut(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

    let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space);
    app.global_shortcut().on_shortcut(shortcut, move |app, _shortcut, event| {
        if event.state() == ShortcutState::Pressed {
            if let Some(window) = app.get_webview_window("main") {
                let is_visible = window.is_visible().unwrap_or(false);
                let is_minimized = window.is_minimized().unwrap_or(false);
                if is_visible && !is_minimized {
                    let _ = window.hide();
                } else {
                    lock_vault_internally(app, "app reopened via shortcut");
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
        }
    })?;
    Ok(())
}

pub fn lock_vault_internally<R: Runtime>(app_handle: &tauri::AppHandle<R>, reason: &str) {
    use tauri::Emitter;
    let state = app_handle.state::<SafeState>();
    let mut s = state.lock().unwrap();
    let is_unlocked = s.vault_state == vaultwarden_core::sync::VaultState::OnlineReady
        || s.vault_state == vaultwarden_core::sync::VaultState::OfflineRead;
    if is_unlocked {
        log::info!("Vault locked automatically: {reason}");
        s.sessions.clear();
        s.user_key = None;
        s.org_keys.clear();
        s.rsa_private_key = None;
        *s.master_password = String::new();
        s.vault_state = vaultwarden_core::sync::VaultState::Locked;
        let _ = app_handle.emit("vault_auto_locked", ());
    }
}

fn setup_auto_lock(app: &mut tauri::App) {
    use std::time::{Duration, Instant};

    let app_handle = app.handle().clone();

    // Load initial persisted settings (e.g. clipboard clear duration)
    let initial_settings = commands::crypto::load_app_settings();
    commands::crypto::CLIPBOARD_CLEAR_SECS.store(
        initial_settings.clipboard_clear_secs,
        std::sync::atomic::Ordering::Relaxed,
    );

    // 1. System sleep / suspend detection loop (runs every second)
    // If the loop slept for more than 3 seconds when 1 second was requested,
    // the computer was suspended or hibernated.
    let app_handle_timer = app_handle.clone();
    std::thread::spawn(move || {
        let mut last_tick = Instant::now();
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let now_instant = Instant::now();
            let elapsed_since_tick = now_instant.duration_since(last_tick);
            last_tick = now_instant;

            if elapsed_since_tick > Duration::from_secs(3) {
                lock_vault_internally(&app_handle_timer, "resumed from system sleep / suspend");
            }
        }
    });

    // 2. Linux OS-level D-Bus sleep & screen-lock listener
    #[cfg(target_os = "linux")]
    {
        let app_handle_dbus = app_handle.clone();
        std::thread::spawn(move || {
            use std::process::{Command, Stdio};
            use std::io::{BufRead, BufReader};

            let child = Command::new("gdbus")
                .args(["monitor", "--system", "--dest", "org.freedesktop.login1"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();

            if let Ok(mut child) = child {
                if let Some(stdout) = child.stdout.take() {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines().map_while(Result::ok) {
                        if line.contains("PrepareForSleep") && line.contains("true") {
                            lock_vault_internally(&app_handle_dbus, "system sleep signal received (D-Bus)");
                        }
                    }
                }
            }
        });

        let app_handle_screensaver = app_handle.clone();
        std::thread::spawn(move || {
            use std::process::{Command, Stdio};
            use std::io::{BufRead, BufReader};

            let child = Command::new("gdbus")
                .args(["monitor", "--session", "--dest", "org.freedesktop.ScreenSaver"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();

            if let Ok(mut child) = child {
                if let Some(stdout) = child.stdout.take() {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines().map_while(Result::ok) {
                        if line.contains("ActiveChanged") && line.contains("true") {
                            lock_vault_internally(&app_handle_screensaver, "screen locked (D-Bus)");
                        }
                    }
                }
            }
        });
    }
}
