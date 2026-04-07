use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::image::Image;
use tauri::webview::WebviewWindowBuilder;
use tauri::{AppHandle, Emitter, Listener, Manager, WebviewUrl};
use tauri::tray::TrayIconId;
use tauri::webview::Color;

use tauri_plugin_window_state::{AppHandleExt, StateFlags};

use crate::watcher::AgentStatus;

static LAST_STATE: Mutex<String> = Mutex::new(String::new());
static PINNED: AtomicBool = AtomicBool::new(false);

fn aggregate_state(agents: &[AgentStatus]) -> &str {
    if agents.is_empty() {
        return "offline";
    }
    let priority = |s: &str| match s {
        "needs-input" => 0,
        "error" => 1,
        "working" => 2,
        "starting" => 3,
        "idle" => 4,
        _ => 5,
    };
    agents
        .iter()
        .min_by_key(|a| priority(&a.status))
        .map(|a| a.status.as_str())
        .unwrap_or("offline")
}

fn icon_bytes(state: &str) -> &'static [u8] {
    match state {
        "needs-input" => include_bytes!("../icons/tray-needs-input.png"),
        "error" => include_bytes!("../icons/tray-error.png"),
        "working" => include_bytes!("../icons/tray-working.png"),
        "starting" => include_bytes!("../icons/tray-starting.png"),
        "idle" => include_bytes!("../icons/tray-idle.png"),
        _ => include_bytes!("../icons/tray-offline.png"),
    }
}

pub fn update_icon(app: &AppHandle, agents: &[AgentStatus]) {
    let state = aggregate_state(agents);

    // Only swap if state changed
    {
        let mut last = LAST_STATE.lock().unwrap_or_else(|e| {
            log::warn!("LAST_STATE mutex poisoned, recovering");
            e.into_inner()
        });
        if last.as_str() == state {
            return;
        }
        *last = state.to_string();
    }

    let bytes = icon_bytes(state);
    if let Ok(icon) = Image::from_bytes(bytes) {
        if let Some(tray) = app.tray_by_id(&TrayIconId::new("main")) {
            let _ = tray.set_icon(Some(icon));
        }
    }
    log::debug!("Tray icon updated to: {}", state);
}

pub fn pin_popup(app: &AppHandle) {
    PINNED.store(true, Ordering::SeqCst);
    let _ = app.emit("pinned-changed", true);

    // Show popup if not already visible
    if let Some(win) = app.get_webview_window("popup") {
        if !win.is_visible().unwrap_or(false) {
            let _ = win.show();
            let _ = win.set_focus();
            emit_current_state(app);
        }
        return;
    }

    // First open — delegate to toggle_popup which creates the window
    toggle_popup(app);
}

pub fn toggle_popup(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("popup") {
        if win.is_visible().unwrap_or(false) {
            PINNED.store(false, Ordering::SeqCst);
            let _ = app.emit("pinned-changed", false);
            let _ = app.save_window_state(StateFlags::POSITION);
            let _ = win.hide();
            return;
        }
        // Already exists but hidden — show at last position and refresh state
        let _ = win.show();
        let _ = win.set_focus();
        emit_current_state(app);
        return;
    }

    // First open: create the popup window
    match WebviewWindowBuilder::new(app, "popup", WebviewUrl::default())
        .title("AgentTray")
        .inner_size(400.0, 120.0) // Minimal; frontend auto-resizes to fit content
        .decorations(false)
        .transparent(true)
        .background_color(Color(0, 0, 0, 0)) // Transparent WebView2 on Windows
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .build()
    {
        Ok(win) => {
            // Set top-right default in physical pixels; plugin restores saved position
            // asynchronously on webview ready and will override this.
            let (default_x, default_y) = compute_default_position(app);
            let _ = win.set_position(tauri::PhysicalPosition::new(default_x, default_y));
            #[cfg(target_os = "linux")]
            {
                let _ = std::process::Command::new("wmctrl")
                    .args(["-r", "AgentTray", "-b", "remove,sticky"])
                    .output();
                let _ = std::process::Command::new("wmctrl")
                    .args(["-r", "AgentTray", "-b", "add,sticky"])
                    .output();
            }
            let _ = win.show();
            let _ = win.set_focus();
            // Emit current state once the frontend signals it has registered
            // its event listeners. The frontend emits 'popup-ready' from
            // onMount after all listeners are active. This avoids the race
            // between WebView load time and state emission.
            let h = app.clone();
            app.once("popup-ready", move |_| {
                emit_current_state(&h);
            });
        }
        Err(e) => log::error!("Failed to create popup window: {}", e),
    }
}

fn emit_current_state(app: &AppHandle) {
    crate::watcher::emit_latest(app);
}

#[tauri::command]
pub fn toggle_pin(app: AppHandle) {
    if PINNED.load(Ordering::SeqCst) {
        PINNED.store(false, Ordering::SeqCst);
        let _ = app.emit("pinned-changed", false);
    } else {
        PINNED.store(true, Ordering::SeqCst);
        let _ = app.emit("pinned-changed", true);
    }
}

pub fn hide_popup(app: &AppHandle) {
    PINNED.store(false, Ordering::SeqCst);
    let _ = app.emit("pinned-changed", false);
    if let Some(win) = app.get_webview_window("popup") {
        let _ = app.save_window_state(StateFlags::POSITION);
        let _ = win.hide();
    }
}

#[tauri::command]
pub fn close_popup(app: AppHandle) {
    hide_popup(&app);
}

fn compute_default_position(app: &AppHandle) -> (i32, i32) {
    app.primary_monitor()
        .ok()
        .flatten()
        .or_else(|| app.available_monitors().ok().and_then(|m| m.into_iter().next()))
        .map(|m| (m.size().width as i32 - 410, 32))
        .unwrap_or((100, 32))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(status: &str) -> AgentStatus {
        AgentStatus {
            id: "test:0".into(),
            name: "test".into(),
            status: status.into(),
            message: "".into(),
            terminal: None,
            can_focus: false,
            cpu: None,
            source: None,
            cli: None,
            session_id: None,
            hook_event: None,
            hook_matcher: None,
            mtime: None,
        }
    }

    #[test]
    fn aggregate_empty_returns_offline() {
        assert_eq!(aggregate_state(&[]), "offline");
    }

    #[test]
    fn aggregate_single_agent() {
        assert_eq!(aggregate_state(&[agent("working")]), "working");
    }

    #[test]
    fn aggregate_needs_input_beats_all() {
        let agents = vec![
            agent("idle"),
            agent("working"),
            agent("needs-input"),
            agent("error"),
        ];
        assert_eq!(aggregate_state(&agents), "needs-input");
    }

    #[test]
    fn aggregate_error_beats_working() {
        let agents = vec![agent("working"), agent("error"), agent("idle")];
        assert_eq!(aggregate_state(&agents), "error");
    }
}
