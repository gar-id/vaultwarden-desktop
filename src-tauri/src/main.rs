#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // WebKitGTK on Linux can render a persistently blank/white window on
    // certain GPU+compositor combos (NVIDIA + DMA-BUF, some Wayland
    // compositors with Intel GPUs too) - see tauri-apps/tauri#9394 and
    // #15050. Must be set before the webview is created, so this runs
    // before vaultwarden_desktop_lib::run() touches any GTK/WebKit API.
    // Only applied if the user hasn't already set an override themselves.
    #[cfg(target_os = "linux")]
    {
        if std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").is_err() {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
        if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }

    vaultwarden_desktop_lib::run();
}
