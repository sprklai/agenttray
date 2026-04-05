#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod focus;
mod focusers;
mod notifier;
mod scanner;
mod tray;
mod watcher;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

fn main() {
    env_logger::init();

    // Log panics from any thread before the process aborts (release uses panic=abort)
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("PANIC: {}", info);
        default_hook(info);
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let handle = app.handle().clone();

            // Spawn watcher on a dedicated OS thread (blocking loop)
            let h_watch = handle.clone();
            std::thread::spawn(move || watcher::watch(h_watch));


            // Build tray menu (required for AppIndicator on GNOME to show icon;
            // GNOME AppIndicator does NOT support direct click events — only menu)
            let h_show = handle.clone();
            let h_hide = handle.clone();
            let show_item = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let pin_item = MenuItem::with_id(app, "pin", "Pin", true, None::<&str>)?;
            let hide_item = MenuItem::with_id(app, "hide", "Hide", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &pin_item, &hide_item, &separator, &quit_item])?;

            TrayIconBuilder::with_id("main")
                .icon(tauri::image::Image::from_bytes(include_bytes!(
                    "../icons/tray-offline.png"
                ))?)
                .tooltip("AgentTray")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event({
                    let h_pin = handle.clone();
                    move |app, event| match event.id.as_ref() {
                        "show" => tray::toggle_popup(&h_show),
                        "pin" => tray::pin_popup(&h_pin),
                        "hide" => tray::hide_popup(&h_hide),
                        "quit" => app.exit(0),
                        _ => {}
                    }
                })
                .on_tray_icon_event(move |_tray, event| {
                    // Works on KDE/Windows/macOS — left-click toggles popup directly
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        tray::toggle_popup(&handle);
                    }
                })
                .build(app)?;

            // Global shortcut: Ctrl+Shift+A toggles the popup
            let h_shortcut = app.handle().clone();
            let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyA);
            app.handle().plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(move |_app, sc, event| {
                        if sc == &shortcut && event.state() == ShortcutState::Pressed {
                            tray::toggle_popup(&h_shortcut);
                        }
                    })
                    .build(),
            )?;
            app.global_shortcut().register(shortcut)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![focus::focus_terminal, watcher::get_agents, watcher::get_status_dir, watcher::install_hooks, tray::toggle_pin])
        .build(tauri::generate_context!())
        .expect("AgentTray failed to build")
        .run(|_app, event| {
            // Prevent exit when the last window is hidden — this is a tray-only app.
            // Only block window-close exits (code == None), not explicit quit (code == Some).
            if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
