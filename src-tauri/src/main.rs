#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod focus;
mod focusers;
mod tray;
mod watcher;

use tauri::tray::{TrayIconBuilder, TrayIconEvent};

fn main() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let handle = app.handle().clone();

            // Spawn watcher on a dedicated OS thread (blocking loop)
            let h_watch = handle.clone();
            std::thread::spawn(move || watcher::watch(h_watch));

            // Build system tray icon
            TrayIconBuilder::with_id("main")
                .icon_as_template(false)
                .icon(tauri::image::Image::from_bytes(include_bytes!(
                    "../icons/tray-offline.png"
                ))?)
                .on_tray_icon_event(move |_tray, event| {
                    if let TrayIconEvent::Click { .. } = event {
                        tray::toggle_popup(&handle);
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![focus::focus_terminal])
        .run(tauri::generate_context!())
        .expect("AgentTray failed to start");
}
