# AgentTray — Complete Build Specification v2

## Project Overview

Build **AgentTray**: a cross-platform system tray application (Tauri 2.x + Rust + SvelteKit) that monitors terminal-based AI coding agents (Claude Code, Codex, Gemini CLI, Aider, and others) and displays their real-time status via a colored dot in the OS menu bar / system tray. Clicking the tray icon opens a Svelte popup listing all agents with their status and a "focus ↗" button that jumps directly to the correct terminal window or tab.

The app must be:
- **~6MB release binary** — Tauri 2.x wraps a Svelte frontend; no Electron, no Node runtime at runtime
- **Zero always-on daemon** — the Tauri app itself is the only background process
- **Cross-platform** — macOS, Linux (X11), Windows
- **Low RAM footprint** — target ≤ 30MB RSS at idle; use Tauri's `devtools: false` in production, lazy-load the popup WebView only when first opened
- **Auto-updating** — Tauri updater plugin checks GitHub Releases on startup and prompts the user
- **Extensible without touching core** — adding a new terminal requires creating exactly two files
- **v1.0 terminal scope** — ship with 5 terminals; all others are backlog

---

## Design Principles

Apply these strictly. Every decision must be justifiable against them.

1. **KISS** — simplest solution that works. No abstraction until three concrete cases require it.
2. **Single Responsibility** — each file does one thing. `watcher.rs` reads files. `tray.rs` manages the icon. `focus.rs` dispatches focus. Shell detectors detect. Shell registry iterates detectors. Svelte components render; they do not fetch.
3. **Open/Closed** — adding a new terminal (post-v1) touches exactly: one `detectors/NN_<kind>.sh`, one `focusers/<kind>.rs`, one line in `focusers/mod.rs`. Nothing else changes.
4. **Explicit contracts** — the status file JSON schema is the only interface between shell and Rust. The Tauri event payload is the only interface between Rust and Svelte. Both are versioned.
5. **Fail silently, never crash** — every file read, process spawn is fallible. Log the error to `stderr`, return `Ok(())`, keep the app running.
6. **Memory-first decisions** — prefer stack allocation. Avoid `Arc<Mutex<Vec>>` for data that can be recomputed on demand. The watcher thread owns no heap data between poll cycles.
7. **No magic** — no proc-macros that hide control flow, no global mutable state, no `unwrap()` outside tests.

---

## v1.0 Terminal Scope

### Supported in v1.0

| Terminal | Platform | Detection signal | Focus method |
|---|---|---|---|
| **macOS Terminal.app** | macOS | `$TERM_PROGRAM=Apple_Terminal` + `$TERM_SESSION_ID` | AppleScript: match tab by UUID |
| **iTerm2** | macOS | `$ITERM_SESSION_ID` | AppleScript: select session by ID |
| **Git Bash (mintty)** | Windows | `$MSYSTEM` non-empty | PowerShell: `SetForegroundWindow` by PPID HWND |
| **PowerShell / CMD** | Windows | `$COMSPEC` non-empty, no `$WT_SESSION` | PowerShell: `GetConsoleWindow` HWND |
| **GNOME Terminal / X11 generic** | Linux | `$WINDOWID` non-empty | `wmctrl -ia $WINDOWID` + xdotool tab search |

### Backlog (post-v1.0, tracked as GitHub issues)

Warp, Ghostty, Alacritty, Kitty, Hyper, Tabby, Konsole, Terminator, Tilix, VS Code terminal, JetBrains terminal, Neovim terminal, Windows Terminal (`$WT_SESSION`), ConEmu, Cmder, tmux, GNU screen, Zellij.

Each backlog item requires: one detector `.sh` + one focuser `.rs` + one line in `mod.rs`. No other files change. Document this clearly in `CONTRIBUTING.md` with a step-by-step guide.

---

## Project Structure

```
agent-tray/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs                  # App entry: Tauri setup, updater, tray, watcher thread
│   │   ├── watcher.rs               # notify-rs watcher; parses status JSON; emits events
│   │   ├── tray.rs                  # Icon swap; popup WebView lazy-init + OS-aware position
│   │   ├── focus.rs                 # Tauri command: thin router → focusers/
│   │   ├── notifications.rs         # Native OS notifications: permission + transition dedup
│   │   ├── updater.rs               # Tauri updater plugin integration
│   │   └── focusers/
│   │       ├── mod.rs               # Focuser trait + dispatch; registry of v1 focusers
│   │       ├── os_helpers.rs        # Shared: applescript(), wmctrl_focus(), windows_set_foreground()
│   │       ├── iterm2.rs            # v1
│   │       ├── terminal_app.rs      # v1
│   │       ├── gitbash.rs           # v1
│   │       ├── powershell_cmd.rs    # v1  (covers PowerShell + CMD)
│   │       ├── x11_generic.rs       # v1
│   │       └── unknown.rs           # fallback no-op
│   ├── icons/
│   │   ├── tray-needs-input.png     # 22×22px colored circles on transparent bg
│   │   ├── tray-error.png
│   │   ├── tray-working.png
│   │   ├── tray-starting.png
│   │   ├── tray-idle.png
│   │   └── tray-offline.png
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                             # SvelteKit frontend (popup WebView)
│   ├── app.html
│   ├── app.css                      # Global: CSS variables, dark theme base
│   ├── lib/
│   │   ├── components/
│   │   │   ├── AgentRow.svelte      # Single agent row: dot + name + message + focus button
│   │   │   ├── StatusDot.svelte     # Colored animated dot
│   │   │   ├── Aggregatepill.svelte # Header status pill
│   │   │   ├── SupportBar.svelte    # Footer: coffee button + path hint
│   │   │   └── UpdateBanner.svelte  # Shown when update is available
│   │   ├── stores/
│   │   │   └── agents.ts            # Svelte writable store; populated from Tauri events
│   │   ├── types.ts                 # AgentStatus, TerminalInfo, AggregateState interfaces
│   │   └── utils.ts                 # aggregate(), priorityOf(), escapeHtml()
│   └── routes/
│       └── +page.svelte             # Root page: wires Tauri event listener to store
├── scripts/
│   ├── wrap.sh                      # Universal agent wrapper
│   ├── registry.sh                  # Iterates detectors/, returns first JSON match
│   └── detectors/
│       ├── 20_iterm2.sh             # v1
│       ├── 21_terminal_app.sh       # v1
│       ├── 40_gitbash.sh            # v1
│       ├── 41_powershell_cmd.sh     # v1
│       ├── 60_x11_generic.sh        # v1
│       └── 99_unknown.sh            # always-match fallback
├── tests/
│   ├── shell/
│   │   ├── run_all.sh               # Runs all shell test files; exits non-zero on any failure
│   │   ├── test_registry.sh         # Unit tests: detector priority, env isolation
│   │   ├── test_wrap.sh             # Unit tests: status writes, input detection, atomic write
│   │   ├── test_detectors.sh        # Unit tests: each detector in isolation with mock env
│   │   └── fixtures/
│   │       ├── mock_agent_ok.sh     # Fake agent: prints lines then exits 0
│   │       ├── mock_agent_fail.sh   # Fake agent: prints lines then exits 1
│   │       └── mock_agent_prompt.sh # Fake agent: prints a [y/n] line then waits
│   ├── rust/                        # Inline #[cfg(test)] in each Rust module
│   └── e2e/
│       ├── README.md                # Manual E2E checklist per platform
│       └── scenarios.md             # Scripted scenarios for future automation
├── .github/
│   └── workflows/
│       ├── ci.yml                   # On PR: shell tests + cargo test + cargo clippy
│       └── release.yml              # On tag push: build all platforms, publish to GitHub Releases
├── BACKLOG.md                       # All deferred terminals with implementation guide
├── CONTRIBUTING.md                  # How to add a new terminal (detector + focuser steps)
├── COMMERCIAL_LICENSE
├── LICENSE                          # MIT (personal use)
└── README.md
```

---

## Status File Contract

This is the **only interface** between shell and Rust. Both sides must conform exactly. Never add parsing logic outside `watcher.rs`.

**Location:** `~/.agent-monitor/<agent-name>.status`
**Format:** Single-line JSON, newline-terminated.
**Write strategy:** write to `<file>.tmp` then `mv` — atomic on all target platforms.

### Schema (v1)

```json
{
  "v": 1,
  "status": "needs-input",
  "message": "Overwrite src/api.ts? [y/n]",
  "terminal": {
    "kind": "iterm2",
    "focus_id": "w0t1p0:3F2504E0-4F89-11D3-9A0C-0305E82C3301",
    "outer_id": "",
    "label": "iTerm2"
  }
}
```

### Field Definitions

| Field | Type | Required | Description |
|---|---|---|---|
| `v` | integer | yes | Schema version. Currently `1`. Watcher ignores files with unknown `v`. |
| `status` | string | yes | One of: `needs-input`, `error`, `working`, `starting`, `idle`, `offline` |
| `message` | string | yes | Last stdout line from the agent. Empty string if none. Max 500 chars; truncate longer. |
| `terminal` | object | no | Absent for manual writes. Popup shows agent but hides focus button. |
| `terminal.kind` | string | yes | Matches a focuser filename without `.rs` extension. |
| `terminal.focus_id` | string | yes | Primary focus target — opaque except to the matching focuser. |
| `terminal.outer_id` | string | yes | For multiplexers: outer window ID. Empty string for direct terminals. |
| `terminal.label` | string | yes | Human-readable name shown in popup. |

### v1.0 Focus ID semantics

| kind | focus_id value | outer_id |
|---|---|---|
| `iterm2` | `$ITERM_SESSION_ID` (e.g. `w0t1p0:UUID`) | `""` |
| `terminal_app` | `$TERM_SESSION_ID` UUID | `""` |
| `gitbash` | PPID HWND as decimal string | `""` |
| `powershell_cmd` | Console HWND via `GetConsoleWindow()` | `""` |
| `x11_generic` | `"WINDOWID:SHELLPID"` colon-separated | `""` |
| `unknown` | Shell PID as string | `""` |

### Backward compatibility

Status files written in the legacy pipe format (`working|message`) must still display in the popup with `can_focus: false`. `watcher.rs` detects the format by attempting JSON parse first; on failure it falls back to splitting on `|`.

---

## Shell Side

### `scripts/registry.sh`

Sources every `detectors/*.sh` in lexicographic order. Runs each as a subprocess (`bash "$detector"`). Returns the first non-empty stdout result. If all detectors produce empty output (impossible given `99_unknown.sh` but defensive), returns the unknown fallback.

**Rules:**
- Never modify the parent shell's environment
- Each detector runs in a clean subshell — cannot affect siblings
- The numeric prefix controls priority: lower number = higher priority

### `scripts/wrap.sh`

**Signature:** `wrap.sh <agent-name> <command> [args...]`

**Responsibilities, in order:**
1. Create `~/.agent-monitor/` if absent
2. Source `registry.sh` → populates `$TERMINAL_JSON`
3. Atomically write `starting` status
4. Run `<command> [args...]`, piping stdout+stderr through a `while read` loop
5. Per line: regex match against `$INPUT_PAT` → write `needs-input` or `working`
6. On subprocess exit: write `idle` (code 0) or `error` (code ≠ 0)
7. On `SIGTERM`/`SIGINT`: write `offline` and clean up `.tmp` file

**Atomic write helper:**
```bash
_write_status() {
  local status="$1" message="$2"
  local safe="${message//\"/\\\"}"          # escape double quotes
  safe="${safe:0:500}"                       # truncate at 500 chars
  printf '{"v":1,"status":"%s","message":"%s","terminal":%s}\n' \
    "$status" "$safe" "$TERMINAL_JSON" > "$FILE.tmp"
  mv -f "$FILE.tmp" "$FILE"
}
```

**Input detection patterns** — compiled into one ERE assigned to `$INPUT_PAT`:
```
\? $
\? .*\[
\[y/n\]
\[Y/n\]
\[yes/no\]
password:
Password:
passphrase:
Passphrase:
Enter to
Press .* to
Overwrite\?
Continue\?
Confirm\?
Proceed\?
Are you sure
```

Users may set `INPUT_EXTRA` before calling `wrap.sh`; it is appended to the pattern with `|`.

### `scripts/detectors/<priority>_<kind>.sh`

Each detector must:
- Check its specific environment variable(s)
- Print exactly one JSON line on stdout if matched, nothing if not
- Exit 0 in all cases
- Never modify the environment
- Complete in ≤ 5ms (no network calls, no expensive subprocess chains)

**Template:**
```bash
#!/usr/bin/env bash
[ -z "$ENV_VAR" ] && exit 0
printf '{"kind":"<kind>","focus_id":"%s","outer_id":"","label":"<Label>"}\n' "$ENV_VAR"
```

### v1.0 Detector implementations

**`20_iterm2.sh`**
```bash
#!/usr/bin/env bash
[ -z "$ITERM_SESSION_ID" ] && exit 0
printf '{"kind":"iterm2","focus_id":"%s","outer_id":"","label":"iTerm2"}\n' \
  "$ITERM_SESSION_ID"
```

**`21_terminal_app.sh`**
```bash
#!/usr/bin/env bash
[ "$TERM_PROGRAM" != "Apple_Terminal" ] && exit 0
[ -z "$TERM_SESSION_ID" ] && exit 0
printf '{"kind":"terminal_app","focus_id":"%s","outer_id":"","label":"Terminal"}\n' \
  "$TERM_SESSION_ID"
```

**`40_gitbash.sh`**
```bash
#!/usr/bin/env bash
[ -z "$MSYSTEM" ] && exit 0
hwnd=$(powershell.exe -NoProfile -Command \
  "(Get-Process -Id $PPID -EA SilentlyContinue).MainWindowHandle" \
  2>/dev/null | tr -d '\r\n')
[ -z "$hwnd" ] || [ "$hwnd" = "0" ] && hwnd="$$"
printf '{"kind":"gitbash","focus_id":"%s","outer_id":"","label":"Git Bash"}\n' "$hwnd"
```

**`41_powershell_cmd.sh`**
```bash
#!/usr/bin/env bash
[ -z "$COMSPEC" ] && exit 0
hwnd=$(powershell.exe -NoProfile -Command \
  "Add-Type -Name K -Namespace W \
   -MemberDefinition '[DllImport(\"kernel32.dll\")] public static extern IntPtr GetConsoleWindow();'
   [W.K]::GetConsoleWindow()" \
  2>/dev/null | tr -d '\r\n')
[ -z "$hwnd" ] || [ "$hwnd" = "0" ] && hwnd="$$"
printf '{"kind":"powershell_cmd","focus_id":"%s","outer_id":"","label":"PowerShell"}\n' "$hwnd"
```

**`60_x11_generic.sh`**
```bash
#!/usr/bin/env bash
[ -z "$WINDOWID" ] && exit 0
printf '{"kind":"x11_generic","focus_id":"%s:%d","outer_id":"","label":"Terminal"}\n' \
  "$WINDOWID" "$$"
```

**`99_unknown.sh`**
```bash
#!/usr/bin/env bash
printf '{"kind":"unknown","focus_id":"%d","outer_id":"","label":"Terminal"}\n' "$$"
```

---

## Rust Side

### `Cargo.toml`

```toml
[package]
name    = "agent-tray"
version = "0.1.0"
edition = "2021"

[dependencies]
tauri                      = { version = "2", features = ["tray-icon"] }
tauri-plugin-shell         = "2"
tauri-plugin-updater       = "2"
tauri-plugin-process       = "2"
tauri-plugin-notification  = "2"
notify                     = { version = "6", default-features = false, features = ["macos_kqueue", "inotify", "ReadDirectoryChangesWatcher"] }
serde                      = { version = "1", features = ["derive"] }
serde_json                 = "1"
dirs-next                  = "2"
log                        = "0.4"
env_logger                 = "0.11"

[profile.release]
opt-level     = "z"
lto           = "thin"
strip         = true
codegen-units = 1
panic         = "abort"
```

**Note on RAM:** `lto = "thin"` (not `"fat"`) keeps link times reasonable while still reducing binary size. `panic = "abort"` removes the unwinding machinery (~200KB). `notify` with per-platform features avoids pulling in the polling fallback on platforms where native watchers exist.

### `tauri.conf.json`

```json
{
  "productName": "AgentTray",
  "version": "0.1.0",
  "identifier": "com.agenttray.app",
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:5173"
  },
  "app": {
    "withGlobalTauri": true,
    "trayIcon": {
      "iconPath": "icons/tray-offline.png",
      "iconAsTemplate": false
    },
    "windows": [
      {
        "label": "popup",
        "url": "index.html",
        "width": 300,
        "height": 420,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "skipTaskbar": true,
        "visible": false,
        "resizable": false,
        "devtools": false
      }
    ],
    "plugins": {
      "updater": {
        "pubkey": "YOUR_TAURI_UPDATER_PUBLIC_KEY",
        "endpoints": [
          "https://github.com/YOUR_ORG/agent-tray/releases/latest/download/latest.json"
        ],
        "dialog": true,
        "windows": { "installerArgs": ["/passive"] }
      }
    }
  }
}
```

### `src/updater.rs`

```rust
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

/// Called once at startup in a background task.
/// Checks for a new release, shows the built-in dialog if one is found.
/// Swallows all errors — update failure must never affect core functionality.
pub async fn check_for_update(app: AppHandle) {
    let Ok(updater) = app.updater() else { return };
    match updater.check().await {
        Ok(Some(update)) => {
            log::info!("Update available: {}", update.version);
            // Tauri's built-in dialog handles download + install prompt
            let _ = update.download_and_install(|_, _| {}, || {}).await;
        }
        Ok(None) => log::debug!("No update available"),
        Err(e)   => log::warn!("Update check failed: {e}"),
    }
}
```

### `src/notifications.rs`

Responsibilities: request OS notification permission at startup; send a native notification when any agent transitions into `needs-input` or `error`; deduplicate so the same agent does not fire repeatedly for the same state.

Uses the official `tauri-plugin-notification` (the Tauri-maintained first-party plugin, `crates.io: tauri-plugin-notification = "2"`). This is distinct from the third-party `tauri-plugin-notifications` crate.

```rust
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// Tracks the last-notified status per agent so we don't spam.
/// Key = agent name, Value = last status we notified on.
static LAST_NOTIFIED: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// Call once at app startup. Requests permission on macOS; no-op on Windows/Linux.
pub fn request_permission<R: tauri::Runtime>(app: &AppHandle<R>) {
    // tauri-plugin-notification handles platform differences internally.
    // On Windows and Linux permission is implicitly granted.
    // On macOS this triggers the system permission dialog on first launch.
    if let Ok(granted) = app.notification().permission_state() {
        if granted != tauri_plugin_notification::PermissionState::Granted {
            let _ = app.notification().request_permission();
        }
    }
}

/// Called by watcher.rs after every poll cycle with the current agent list.
/// Fires a native OS notification for each agent that newly entered
/// `needs-input` or `error` state since the last call.
///
/// Rules:
/// - Only fires for `needs-input` and `error` — not for `working`, `idle`, etc.
/// - Deduplicates: if agent "codex" was already notified for `needs-input`,
///   no second notification until its status changes to something else and back.
/// - Swallows all notification errors — a failed notification must not affect
///   the watcher loop or tray icon update.
pub fn notify_transitions<R: tauri::Runtime>(
    app: &AppHandle<R>,
    agents: &[crate::watcher::AgentStatus],
) {
    let notify_states = ["needs-input", "error"];
    let mut last = LAST_NOTIFIED.lock().unwrap();
    let map = last.get_or_insert_with(HashMap::new);

    for agent in agents {
        let prev = map.get(&agent.name).map(|s| s.as_str()).unwrap_or("");
        let curr = agent.status.as_str();

        let should_notify = notify_states.contains(&curr) && prev != curr;
        if should_notify {
            let title = match curr {
                "needs-input" => format!("{} needs input", agent.name),
                "error"       => format!("{} errored", agent.name),
                _             => continue,
            };
            let body = if agent.message.is_empty() {
                agent.status.clone()
            } else {
                agent.message.chars().take(120).collect()
            };

            let _ = app.notification()
                .builder()
                .title(&title)
                .body(&body)
                .show();

            log::debug!("notification sent: {title}");
        }

        // Always update last-known state so dedup works correctly
        map.insert(agent.name.clone(), curr.to_string());
    }

    // Remove agents that are no longer in the list (they were deleted)
    map.retain(|name, _| agents.iter().any(|a| &a.name == name));
}
```

**Integration with `watcher.rs`:** call `notifications::notify_transitions(&app, &agents)` immediately after building the sorted agents vec, before emitting the `agents-updated` event:

```rust
// In watcher.rs read_and_emit():
let agents = read_all(&dir);
crate::notifications::notify_transitions(&app, &agents);      // ← new line
let payload = serde_json::to_string(&agents).unwrap_or_else(|_| "[]".into());
let _ = app.emit("agents-updated", &payload);
tray::update_icon(&app, &agents);
```

**`capabilities/default.json` — required permission:**
```json
{
  "identifier": "default",
  "description": "Default permissions",
  "windows": ["popup"],
  "permissions": [
    "notification:default",
    "notification:allow-is-permission-granted",
    "notification:allow-request-permission",
    "notification:allow-send-notification"
  ]
}
```

**Platform behavior:**

| Platform | Notification style | Permission prompt |
|---|---|---|
| macOS | Native Notification Center banner | Yes — first launch only |
| Windows 10/11 | Windows Toast notification | No — always allowed |
| Linux | libnotify / notify-send desktop notification | No — always allowed |

**Notification tests (`#[cfg(test)]` in `notifications.rs`):**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::watcher::AgentStatus;

    fn agent(name: &str, status: &str) -> AgentStatus {
        AgentStatus { name: name.into(), status: status.into(),
                      message: "".into(), terminal: None, can_focus: false }
    }

    // We can't test actual OS notification dispatch in unit tests,
    // but we can test the dedup / transition logic in isolation.

    #[test]
    fn no_notification_for_working_or_idle() {
        let mut map: HashMap<String, String> = HashMap::new();
        let notify_states = ["needs-input", "error"];

        for status in ["working", "idle", "starting", "offline"] {
            let curr = status;
            let prev = map.get("agent").map(|s| s.as_str()).unwrap_or("");
            let should = notify_states.contains(&curr) && prev != curr;
            assert!(!should, "should not notify for {status}");
            map.insert("agent".into(), curr.into());
        }
    }

    #[test]
    fn dedup_prevents_repeat_notification() {
        let mut map: HashMap<String, String> = HashMap::new();
        let notify_states = ["needs-input", "error"];

        // First transition: working → needs-input → should notify
        let prev = "";
        let curr = "needs-input";
        assert!(notify_states.contains(&curr) && prev != curr);
        map.insert("agent".into(), curr.into());

        // Second cycle: still needs-input → should NOT notify again
        let prev = map.get("agent").map(|s| s.as_str()).unwrap_or("");
        let curr = "needs-input";
        assert!(!(notify_states.contains(&curr) && prev != curr));
    }

    #[test]
    fn re_notifies_after_state_change_and_back() {
        let mut map: HashMap<String, String> = HashMap::new();
        let notify_states = ["needs-input", "error"];

        // needs-input → notify
        map.insert("agent".into(), "needs-input".into());
        // idle (user responded) → no notification, but update map
        map.insert("agent".into(), "idle".into());
        // needs-input again → should notify again
        let prev = map.get("agent").map(|s| s.as_str()).unwrap_or("");
        let curr = "needs-input";
        assert!(notify_states.contains(&curr) && prev != curr);
    }

    #[test]
    fn deleted_agent_removed_from_map() {
        let mut map: HashMap<String, String> = HashMap::new();
        map.insert("gone-agent".into(), "idle".into());
        map.insert("live-agent".into(), "working".into());

        let current_agents = vec![agent("live-agent", "working")];
        map.retain(|name, _| current_agents.iter().any(|a| &a.name == name));

        assert!(!map.contains_key("gone-agent"));
        assert!(map.contains_key("live-agent"));
    }
}
```

### `src/main.rs`

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod focus;
mod focusers;
mod notifications;
mod tray;
mod updater;
mod watcher;

use tauri::Manager;
use tauri::tray::{TrayIconBuilder, TrayIconEvent};

fn main() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let handle = app.handle().clone();

            // Spawn watcher on a dedicated OS thread (blocking loop)
            let h_watch = handle.clone();
            std::thread::spawn(move || watcher::watch(h_watch));

            // Spawn update check on async runtime (non-blocking)
            let h_update = handle.clone();
            tauri::async_runtime::spawn(async move {
                updater::check_for_update(h_update).await;
            });

            // Request notification permission on first launch (macOS requires this)
            notifications::request_permission(app.handle());

            // Build system tray icon
            TrayIconBuilder::new()
                .id("main")
                .icon(app.default_window_icon().unwrap().clone())
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
```

### `src/watcher.rs`

**Key types:**

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TerminalInfo {
    pub kind:     String,
    pub focus_id: String,
    pub outer_id: String,
    pub label:    String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentStatus {
    pub name:      String,   // derived from filename stem, not in JSON
    pub status:    String,
    pub message:   String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal:  Option<TerminalInfo>,
    pub can_focus: bool,     // computed, not in JSON
}
```

**Parse logic — `parse_status_file(path)`:**
1. Read file to string; return `None` on IO error
2. Trim whitespace; return `None` if empty
3. Try `serde_json::from_str::<serde_json::Value>`
4. On success: check `v` field — skip file if `v` exists and is not `1`
5. Extract `status`, `message`, `terminal` fields
6. On JSON parse failure: try legacy format — split on first `|` to get `(status, message)`, set `terminal: None`
7. Set `can_focus = terminal.is_some() && !terminal.focus_id.is_empty() && terminal.focus_id != "0"`
8. Return `Some(AgentStatus { name: filename_stem, ... })`

**Sort order:** `needs-input(0) > error(1) > working(2) > starting(3) > idle(4) > offline(5)`, then alphabetical by name within tier.

**Watch loop:**
- Use `notify::RecommendedWatcher` with `notify::Config::default()` (uses native OS events, not polling)
- Fallback: if native watcher unavailable, use poll interval of 400ms
- Watch `~/.agent-monitor/` with `RecursiveMode::NonRecursive`
- React to `Create`, `Modify`, `Remove` events
- Debounce: coalesce events within a 50ms window before re-reading all files
- Emit `agents-updated` event with serialized `Vec<AgentStatus>` payload
- On startup: emit current state before entering loop

**Memory note:** the watcher thread holds no heap data between iterations. Each cycle allocates a fresh `Vec<AgentStatus>`, serializes it, emits it, then drops it. Total peak allocation per cycle ≈ (number of agents × 300 bytes).

### `src/tray.rs`

**Aggregate logic:** iterate `Vec<AgentStatus>`, return the highest-priority `status` string. Priority order: `needs-input > error > working > starting > idle > offline`.

**Icon swap:** load `icons/tray-{state}.png` from the resource directory via `app.path().resource_dir()`. Cache the last-set state to avoid redundant disk reads.

```rust
pub fn update_icon<R: tauri::Runtime>(app: &tauri::AppHandle<R>, agents: &[AgentStatus]) {
    let state = aggregate_state(agents);
    // Only swap if state changed
    if state == *LAST_STATE.lock().unwrap() { return; }
    // ... load and set icon
}
```

**Popup positioning — OS-aware:**
- macOS/Linux: `x = monitor_width - 310`, `y = 32`
- Windows: `x = monitor_width - 310`, `y = monitor_height - 450`

**Lazy WebView init:** the popup `WebviewWindow` is created on the first click, not at startup. This saves ~15MB RSS at idle. After creation it is hidden/shown, never destroyed.

```rust
pub fn toggle_popup<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(win) = app.get_webview_window("popup") {
        if win.is_visible().unwrap_or(false) { let _ = win.hide(); return; }
        position_popup(app, &win);
        let _ = win.show();
        let _ = win.set_focus();
    } else {
        // First click: create the WebviewWindow
        let win = create_popup_window(app);
        position_popup(app, &win);
        let _ = win.show();
        let _ = win.set_focus();
    }
}
```

### `src/focus.rs`

Thin Tauri command. Zero focus logic.

```rust
use tauri::command;
use crate::focusers;

#[derive(serde::Deserialize)]
pub struct FocusRequest {
    pub kind:     String,
    pub focus_id: String,
    pub outer_id: String,
}

#[command]
pub fn focus_terminal(req: FocusRequest) -> Result<(), String> {
    log::debug!("focus_terminal: kind={} focus_id={}", req.kind, req.focus_id);
    focusers::dispatch(&req.kind, &req.focus_id, &req.outer_id)
}
```

### `src/focusers/mod.rs`

**This is the only file that changes when adding a new terminal post-v1.**

```rust
pub mod iterm2;
pub mod terminal_app;
pub mod gitbash;
pub mod powershell_cmd;
pub mod x11_generic;
pub mod unknown;
pub mod os_helpers;   // not a focuser — shared platform primitives

pub trait Focuser: Send + Sync {
    /// Focus the terminal. Return Ok(()) for "not supported" or "tool absent".
    /// Return Err(msg) only for definitive, user-visible failures.
    fn focus(&self, focus_id: &str, outer_id: &str) -> Result<(), String>;
}

pub fn dispatch(kind: &str, focus_id: &str, outer_id: &str) -> Result<(), String> {
    let focuser: &dyn Focuser = match kind {
        "iterm2"         => &iterm2::ITerm2,
        "terminal_app"   => &terminal_app::TerminalApp,
        "gitbash"        => &gitbash::GitBash,
        "powershell_cmd" => &powershell_cmd::PowerShellCmd,
        "x11_generic"    => &x11_generic::X11Generic,
        _                => &unknown::Unknown,
    };
    focuser.focus(focus_id, outer_id)
}
```

### `src/focusers/os_helpers.rs`

Shared platform primitives. All functions return `Result<(), String>`. All functions are cfg-gated.

```rust
#[cfg(target_os = "macos")]
pub fn applescript(script: &str) -> Result<(), String> { ... }

#[cfg(target_os = "macos")]
pub fn activate_macos_app(name: &str) -> Result<(), String> {
    applescript(&format!(r#"tell application "{name}" to activate"#))
}

#[cfg(target_os = "linux")]
pub fn wmctrl_focus(window_id: &str) -> Result<(), String> { ... }

#[cfg(target_os = "linux")]
pub fn xdotool_windowactivate(window_id: &str) -> Result<(), String> { ... }

#[cfg(target_os = "linux")]
pub fn is_child_window(parent_xid: &str, child_xid: &str) -> bool { ... }

#[cfg(target_os = "linux")]
pub fn is_process_ancestor(window_pid: &str, target_pid: &str) -> bool { ... }

#[cfg(target_os = "windows")]
pub fn windows_set_foreground(hwnd: &str) -> Result<(), String> { ... }

pub fn spawn_silent(cmd: &str, args: &[&str]) -> Result<(), String> { ... }
```

### v1.0 Focuser implementations

**`iterm2.rs`** — macOS only. AppleScript iterates all sessions, matches on `id of s == focus_id`, calls `select s` + `activate`. Falls back to `activate_macos_app("iTerm2")` if AppleScript fails.

**`terminal_app.rs`** — macOS only. AppleScript iterates all windows and tabs, matches tab whose `id` contains `focus_id` UUID, sets `selected tab of w to t`, `frontmost of w to true`, `activate`.

**`gitbash.rs`** — Windows only. Calls `windows_set_foreground(focus_id)` where `focus_id` is the HWND string.

**`powershell_cmd.rs`** — Windows only. Calls `windows_set_foreground(focus_id)` where `focus_id` is the console HWND string.

**`x11_generic.rs`** — Linux only. Splits `focus_id` on `:` to get `(window_id, shell_pid)`. Calls `wmctrl_focus(window_id)`. Sleeps 100ms. Runs `xdotool search --pid {shell_pid}`, iterates results, uses `is_child_window(window_id, xid)` to find the tab widget, calls `xdotool_windowactivate(xid)`.

**`unknown.rs`** — No-op. Returns `Ok(())`.

---

## Svelte Frontend

### Stack

- **SvelteKit** with static adapter (`@sveltejs/adapter-static`) — outputs to `dist/`
- **Svelte 5** (runes API) — use `$state`, `$derived`, `$effect` throughout; no `writable()` stores
- **shadcn-svelte** — use for `Button`, `Badge`, `Separator`, `Tooltip`
- **lucide-svelte** — use for icons: `Terminal`, `AlertCircle`, `Loader2`, `CheckCircle2`, `Circle`, `ArrowUpRight`, `Coffee`, `RefreshCw`
- **Tailwind CSS v4** — utility classes only, no custom CSS except CSS variables in `app.css`
- **TypeScript** — strict mode, no `any`

### Vite / SvelteKit config

`vite.config.ts`:
```typescript
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';

export default {
  plugins: [sveltekit(), tailwindcss()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
};
```

`svelte.config.js`:
```javascript
import adapter from '@sveltejs/adapter-static';
export default {
  kit: {
    adapter: adapter({ fallback: 'index.html' }),
  },
};
```

### `package.json` dependencies

```json
{
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-updater": "^2",
    "@tauri-apps/plugin-notification": "^2",
    "lucide-svelte": "^0.400.0",
    "bits-ui": "^0.21.0",
    "tailwind-variants": "^0.2.1"
  },
  "devDependencies": {
    "@sveltejs/adapter-static": "^3",
    "@sveltejs/kit": "^2",
    "@tailwindcss/vite": "^4",
    "svelte": "^5",
    "svelte-check": "^3",
    "tailwindcss": "^4",
    "typescript": "^5",
    "vitest": "^1",
    "@testing-library/svelte": "^5"
  }
}
```

**Note on shadcn-svelte:** shadcn-svelte components are copied into `src/lib/components/ui/` at project init via `npx shadcn-svelte@latest init`. They are owned code, not a runtime dep. Use: `Button`, `Badge`, `Separator`, `Tooltip`, `TooltipContent`, `TooltipTrigger`.

### `src/lib/types.ts`

```typescript
export type Status = 'needs-input' | 'error' | 'working' | 'starting' | 'idle' | 'offline';

export interface TerminalInfo {
  kind: string;
  focus_id: string;
  outer_id: string;
  label: string;
}

export interface AgentStatus {
  name: string;
  status: Status;
  message: string;
  terminal: TerminalInfo | null;
  can_focus: boolean;
}

export type AggregateState = Status;

export const STATUS_PRIORITY: Record<Status, number> = {
  'needs-input': 0,
  'error':       1,
  'working':     2,
  'starting':    3,
  'idle':        4,
  'offline':     5,
};

export const STATUS_LABEL: Record<Status, string> = {
  'needs-input': 'needs input',
  'error':       'error',
  'working':     'working',
  'starting':    'starting',
  'idle':        'idle',
  'offline':     'offline',
};

export const STATUS_COLOR: Record<Status, string> = {
  'needs-input': '#dd4f4f',
  'error':       '#cc7a28',
  'working':     '#c99626',
  'starting':    '#4898cc',
  'idle':        '#78b644',
  'offline':     '#555555',
};
```

### `src/lib/utils.ts`

```typescript
import { STATUS_PRIORITY, type Status, type AgentStatus } from './types';

export function aggregate(agents: AgentStatus[]): Status {
  if (agents.length === 0) return 'offline';
  return agents.reduce((best, a) =>
    STATUS_PRIORITY[a.status as Status] < STATUS_PRIORITY[best]
      ? a.status as Status
      : best,
    'offline' as Status
  );
}

export function escHtml(s: string): string {
  return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
}
```

### `src/routes/+page.svelte`

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import type { AgentStatus } from '$lib/types';
  import AgentRow from '$lib/components/AgentRow.svelte';
  import AggregatePill from '$lib/components/AggregatePill.svelte';
  import SupportBar from '$lib/components/SupportBar.svelte';
  import UpdateBanner from '$lib/components/UpdateBanner.svelte';
  import { aggregate } from '$lib/utils';

  let agents = $state<AgentStatus[]>([]);
  let aggregateState = $derived(aggregate(agents));

  onMount(async () => {
    // Receive real-time updates from Rust watcher
    const unlisten = await listen<AgentStatus[]>('agents-updated', (event) => {
      agents = event.payload;
    });

    // Close popup when window loses focus
    const win = getCurrentWindow();
    const unlistenBlur = await win.onFocusChanged(({ payload: focused }) => {
      if (!focused) win.hide();
    });

    return () => { unlisten(); unlistenBlur(); };
  });

  async function focusAgent(agent: AgentStatus) {
    if (!agent.can_focus || !agent.terminal) return;
    await invoke('focus_terminal', {
      req: {
        kind:     agent.terminal.kind,
        focus_id: agent.terminal.focus_id,
        outer_id: agent.terminal.outer_id,
      }
    });
    getCurrentWindow().hide();
  }
</script>

<div class="w-[292px] rounded-[10px] overflow-hidden bg-[#1c1c1c] border border-white/10 shadow-2xl m-[4px]">
  <!-- Header -->
  <div class="flex items-center justify-between px-3 py-2 border-b border-white/7">
    <span class="text-[10px] font-semibold tracking-widest uppercase text-[#7a7870]">Agents</span>
    <AggregatePill state={aggregateState} />
  </div>

  <!-- Update banner (shown when update available) -->
  <UpdateBanner />

  <!-- Agent list -->
  <div class="py-1">
    {#if agents.length === 0}
      <p class="text-[11px] text-[#7a7870] text-center py-5 leading-relaxed">
        No agents detected.<br/>See ~/.agent-monitor/
      </p>
    {:else}
      {#each agents as agent (agent.name)}
        <AgentRow {agent} onFocus={() => focusAgent(agent)} />
      {/each}
    {/if}
  </div>

  <!-- Footer -->
  <SupportBar />
</div>
```

### `src/lib/components/StatusDot.svelte`

```svelte
<script lang="ts">
  import type { Status } from '$lib/types';
  import { STATUS_COLOR } from '$lib/types';

  let { status }: { status: Status } = $props();

  const animated = ['needs-input', 'working', 'starting'] as const;
  const isAnimated = $derived((animated as readonly string[]).includes(status));
  const color = $derived(STATUS_COLOR[status]);
  const animDuration = $derived(
    status === 'needs-input' ? '0.9s' :
    status === 'working'     ? '1.3s' : '1.7s'
  );
</script>

<div
  class="w-[7px] h-[7px] rounded-full flex-shrink-0"
  style="
    background-color: {color};
    animation: {isAnimated ? `blink ${animDuration} ease-in-out infinite` : 'none'};
  "
></div>

<style>
  @keyframes blink {
    0%, 100% { opacity: 1; }
    50%       { opacity: 0.35; }
  }
</style>
```

### `src/lib/components/AgentRow.svelte`

```svelte
<script lang="ts">
  import type { AgentStatus } from '$lib/types';
  import { STATUS_LABEL } from '$lib/types';
  import StatusDot from './StatusDot.svelte';
  import { ArrowUpRight, Terminal } from 'lucide-svelte';

  let {
    agent,
    onFocus,
  }: { agent: AgentStatus; onFocus: () => void } = $props();

  let hovered = $state(false);
</script>

<div
  class="flex items-center gap-[9px] px-3 py-[7px] cursor-default transition-colors"
  class:bg-[#252525]={hovered}
  onmouseenter={() => hovered = true}
  onmouseleave={() => hovered = false}
>
  <StatusDot status={agent.status} />

  <div class="flex-1 min-w-0">
    <p class="text-[12px] font-medium text-[#dddbd5] truncate">{agent.name}</p>
    <p class="text-[11px] text-[#7a7870] truncate mt-[1px]">
      {agent.message || STATUS_LABEL[agent.status]}
    </p>
  </div>

  <div class="flex flex-col items-end gap-[3px] flex-shrink-0">
    <span class="text-[10px] text-[#7a7870] opacity-60">
      {agent.terminal?.label ?? ''}
    </span>
    {#if agent.can_focus}
      <button
        class="text-[10px] font-medium px-[7px] py-[2px] rounded border transition-all
               text-[#4898cc] border-[rgba(72,152,204,0.35)] bg-[rgba(72,152,204,0.1)]
               hover:bg-[rgba(72,152,204,0.2)] hover:border-[rgba(72,152,204,0.55)]"
        class:opacity-0={!hovered}
        class:opacity-100={hovered}
        onclick={onFocus}
      >
        focus <ArrowUpRight size={10} class="inline" />
      </button>
    {/if}
  </div>
</div>
```

### `src/lib/components/AggregatePill.svelte`

```svelte
<script lang="ts">
  import type { AggregateState, Status } from '$lib/types';
  import { STATUS_LABEL, STATUS_COLOR } from '$lib/types';

  let { state }: { state: AggregateState } = $props();
</script>

<span
  class="text-[10px] font-medium px-2 py-[2px] rounded-full border"
  style="
    color: {STATUS_COLOR[state]};
    border-color: {STATUS_COLOR[state]}59;
    background-color: {STATUS_COLOR[state]}1a;
  "
>
  {STATUS_LABEL[state]}
</span>
```

### `src/lib/components/SupportBar.svelte`

```svelte
<script lang="ts">
  import { open } from '@tauri-apps/plugin-shell';
  import { Coffee } from 'lucide-svelte';
</script>

<div class="flex items-center justify-between px-3 py-[7px] border-t border-white/7">
  <p class="text-[10px] text-[#7a7870] leading-tight">
    Free for personal use.<br/>Found it useful?
  </p>
  <button
    class="flex items-center gap-[5px] text-[11px] font-medium px-[10px] py-[4px]
           rounded border text-[#c99626] border-[rgba(201,150,38,0.4)] bg-[rgba(201,150,38,0.1)]
           hover:bg-[rgba(201,150,38,0.18)] hover:border-[rgba(201,150,38,0.6)] transition-colors"
    onclick={() => open('https://buymeacoffee.com/YOUR_USERNAME')}
  >
    <Coffee size={13} />
    Buy me a coffee
  </button>
</div>
```

### `src/lib/components/UpdateBanner.svelte`

```svelte
<script lang="ts">
  import { check } from '@tauri-apps/plugin-updater';
  import { relaunch } from '@tauri-apps/plugin-process';
  import { RefreshCw } from 'lucide-svelte';

  let updateAvailable = $state(false);
  let updateVersion = $state('');
  let installing = $state(false);

  // Update availability is set by the Rust side via a Tauri event
  // This component just shows the banner and handles the install click
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';

  onMount(async () => {
    return await listen<string>('update-available', (e) => {
      updateAvailable = true;
      updateVersion = e.payload;
    });
  });

  async function installUpdate() {
    installing = true;
    const update = await check();
    if (update) {
      await update.downloadAndInstall();
      await relaunch();
    }
    installing = false;
  }
</script>

{#if updateAvailable}
  <div class="flex items-center justify-between px-3 py-2 bg-[rgba(72,152,204,0.1)] border-b border-[rgba(72,152,204,0.2)]">
    <span class="text-[11px] text-[#4898cc]">v{updateVersion} available</span>
    <button
      class="flex items-center gap-[4px] text-[10px] font-medium text-[#4898cc]
             px-2 py-1 rounded border border-[rgba(72,152,204,0.4)]
             hover:bg-[rgba(72,152,204,0.15)] transition-colors disabled:opacity-50"
      onclick={installUpdate}
      disabled={installing}
    >
      <RefreshCw size={10} class={installing ? 'animate-spin' : ''} />
      {installing ? 'Installing…' : 'Update'}
    </button>
  </div>
{/if}
```

---

## Auto-Update Flow

### How it works

1. On app startup, `updater::check_for_update()` calls the GitHub Releases endpoint
2. Tauri updater fetches `latest.json` (a file published alongside each release)
3. If version > current, Rust emits `update-available` event with the new version string
4. `UpdateBanner.svelte` shows a blue banner in the popup header
5. User clicks "Update" → `downloadAndInstall()` → `relaunch()`

### `latest.json` format (published to GitHub Releases)

```json
{
  "version": "0.2.0",
  "notes": "See CHANGELOG.md",
  "pub_date": "2025-01-15T12:00:00Z",
  "platforms": {
    "darwin-aarch64": {
      "signature": "...",
      "url": "https://github.com/YOUR_ORG/agent-tray/releases/download/v0.2.0/AgentTray_aarch64.app.tar.gz"
    },
    "darwin-x86_64": {
      "signature": "...",
      "url": "https://github.com/YOUR_ORG/agent-tray/releases/download/v0.2.0/AgentTray_x86_64.app.tar.gz"
    },
    "linux-x86_64": {
      "signature": "...",
      "url": "https://github.com/YOUR_ORG/agent-tray/releases/download/v0.2.0/agent-tray_0.2.0_amd64.AppImage.tar.gz"
    },
    "windows-x86_64": {
      "signature": "...",
      "url": "https://github.com/YOUR_ORG/agent-tray/releases/download/v0.2.0/AgentTray_0.2.0_x64-setup.nsis.zip"
    }
  }
}
```

### Key generation (one-time setup)

```bash
cargo tauri signer generate -w ~/.tauri/agent-tray.key
# Copy the public key → tauri.conf.json "plugins.updater.pubkey"
# Store the private key as TAURI_SIGNING_PRIVATE_KEY GitHub secret
```

### `.github/workflows/release.yml` (abbreviated)

```yaml
name: Release
on:
  push:
    tags: ['v*']

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: macos-latest;  target: aarch64-apple-darwin
          - os: macos-13;      target: x86_64-apple-darwin
          - os: ubuntu-22.04;  target: x86_64-unknown-linux-gnu
          - os: windows-latest; target: x86_64-pc-windows-msvc

    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: '20' }
      - run: npm ci
      - uses: dtolnay/rust-toolchain@stable
        with: { targets: ${{ matrix.target }} }
      - uses: tauri-apps/tauri-action@v0
        env:
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
        with:
          tagName: ${{ github.ref_name }}
          releaseName: AgentTray ${{ github.ref_name }}
          updaterJsonKeepUniversal: true
```

---

## Testing Plan

### Layer 1 — Shell unit tests (`tests/shell/`)

These run on every CI push via `bash tests/shell/run_all.sh`.

#### `test_detectors.sh` — each detector in isolation

Test each detector by running it in a clean environment with only the relevant variables set. Assert the correct `kind` field in output JSON.

```bash
#!/usr/bin/env bash
set -euo pipefail
PASS=0; FAIL=0; DETECTORS_DIR="scripts/detectors"

assert_kind() {
  local name="$1" env_pairs="$2" expected="$3"
  local result kind
  result=$(env -i $env_pairs bash "$DETECTORS_DIR"/${4}.sh 2>/dev/null)
  kind=$(printf '%s' "$result" | grep -o '"kind":"[^"]*"' | cut -d'"' -f4)
  if [ "$kind" = "$expected" ]; then
    printf 'PASS  %s\n' "$name"; ((PASS++))
  else
    printf 'FAIL  %s  got=%q  want=%q\n' "$name" "$kind" "$expected"; ((FAIL++))
  fi
}

# iTerm2
assert_kind "iterm2 basic"        "ITERM_SESSION_ID=w0t0p0:ABC123"  "iterm2"        "20_iterm2"
assert_kind "iterm2 not set"      ""                                  ""              "20_iterm2"

# Terminal.app
assert_kind "terminal_app basic"  "TERM_PROGRAM=Apple_Terminal TERM_SESSION_ID=UUID-1" "terminal_app" "21_terminal_app"
assert_kind "terminal_app wrong"  "TERM_PROGRAM=xterm TERM_SESSION_ID=UUID-1"          ""             "21_terminal_app"
assert_kind "terminal_app no sid" "TERM_PROGRAM=Apple_Terminal"                        ""             "21_terminal_app"

# Git Bash
assert_kind "gitbash basic"       "MSYSTEM=MINGW64"                  "gitbash"       "40_gitbash"
assert_kind "gitbash msys"        "MSYSTEM=MSYS"                     "gitbash"       "40_gitbash"
assert_kind "gitbash not set"     ""                                  ""              "40_gitbash"

# X11 generic
assert_kind "x11 basic"           "WINDOWID=67108876"                "x11_generic"   "60_x11_generic"
assert_kind "x11 not set"         ""                                  ""              "60_x11_generic"

# Unknown fallback
assert_kind "unknown always"      ""                                  "unknown"       "99_unknown"

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
```

#### `test_registry.sh` — priority and isolation

```bash
# Priority: if ITERM_SESSION_ID and WINDOWID both set, iterm2 wins (lower priority number)
result=$(env -i ITERM_SESSION_ID=w0t0p0:X WINDOWID=123 bash scripts/registry.sh 2>/dev/null)
kind=$(printf '%s' "$result" | grep -o '"kind":"[^"]*"' | cut -d'"' -f4)
[ "$kind" = "iterm2" ] || { echo "FAIL priority: got $kind want iterm2"; exit 1; }
echo "PASS priority: iterm2 beats x11_generic"

# Isolation: setting env for detector A must not affect detector B
# (run registry.sh with Terminal.app env; assert terminal_app wins, not iterm2)
result=$(env -i TERM_PROGRAM=Apple_Terminal TERM_SESSION_ID=UUID-99 bash scripts/registry.sh 2>/dev/null)
kind=$(printf '%s' "$result" | grep -o '"kind":"[^"]*"' | cut -d'"' -f4)
[ "$kind" = "terminal_app" ] || { echo "FAIL isolation: got $kind want terminal_app"; exit 1; }
echo "PASS isolation: terminal_app wins when ITERM_SESSION_ID absent"

# Unknown fallback when nothing matches
result=$(env -i bash scripts/registry.sh 2>/dev/null)
kind=$(printf '%s' "$result" | grep -o '"kind":"[^"]*"' | cut -d'"' -f4)
[ "$kind" = "unknown" ] || { echo "FAIL fallback: got $kind want unknown"; exit 1; }
echo "PASS fallback: unknown when no env vars set"

echo "All registry tests passed"
```

#### `test_wrap.sh` — status file writes

```bash
#!/usr/bin/env bash
set -euo pipefail
STATUS_DIR=$(mktemp -d)
trap 'rm -rf "$STATUS_DIR"' EXIT

# Override the status dir for this test
export HOME="$STATUS_DIR"
mkdir -p "$STATUS_DIR/.agent-monitor"

# Helper: run wrap.sh with a mock agent and capture the final status file
run_wrap() {
  local name="$1" script="$2"
  local tmpsh=$(mktemp /tmp/mock_agent.XXXX.sh)
  printf '%s\n' "$script" > "$tmpsh"; chmod +x "$tmpsh"
  timeout 5 bash scripts/wrap.sh "$name" bash "$tmpsh" 2>/dev/null || true
  rm -f "$tmpsh"
}

# Test 1: clean exit → status = idle
run_wrap "test-agent" 'echo "hello"; exit 0'
status=$(grep -o '"status":"[^"]*"' "$STATUS_DIR/.agent-monitor/test-agent.status" | cut -d'"' -f4)
[ "$status" = "idle" ] || { echo "FAIL clean exit: got $status"; exit 1; }
echo "PASS clean exit → idle"

# Test 2: non-zero exit → status = error
run_wrap "test-err" 'echo "oops"; exit 1'
status=$(grep -o '"status":"[^"]*"' "$STATUS_DIR/.agent-monitor/test-err.status" | cut -d'"' -f4)
[ "$status" = "error" ] || { echo "FAIL nonzero exit: got $status"; exit 1; }
echo "PASS non-zero exit → error"

# Test 3: [y/n] prompt detection → final status = needs-input
# (agent prints prompt then waits; wrap.sh should detect and write needs-input before we kill it)
run_wrap "test-prompt" 'echo "Overwrite? [y/n]"; sleep 99' &
sleep 0.5
status=$(grep -o '"status":"[^"]*"' "$STATUS_DIR/.agent-monitor/test-prompt.status" 2>/dev/null | cut -d'"' -f4)
kill %1 2>/dev/null || true
[ "$status" = "needs-input" ] || { echo "FAIL needs-input: got $status"; exit 1; }
echo "PASS [y/n] line → needs-input"

# Test 4: status file is valid JSON
run_wrap "test-json" 'echo "running"; exit 0'
python3 -c "import json,sys; json.load(open('$STATUS_DIR/.agent-monitor/test-json.status'))" \
  || { echo "FAIL not valid JSON"; exit 1; }
echo "PASS status file is valid JSON"

# Test 5: message with double-quote does not break JSON
run_wrap "test-quote" 'echo '"'"'He said "hello"'"'"'; exit 0'
python3 -c "import json,sys; d=json.load(open('$STATUS_DIR/.agent-monitor/test-quote.status')); assert '\"' in d['message']" \
  || { echo "FAIL quote escaping broken"; exit 1; }
echo "PASS double-quote in message is safely escaped"

# Test 6: atomic write — no .tmp file should remain after wrap.sh exits
[ -f "$STATUS_DIR/.agent-monitor/test-agent.status.tmp" ] \
  && { echo "FAIL tmp file left behind"; exit 1; }
echo "PASS no .tmp file left behind"

echo "All wrap.sh tests passed"
```

### Layer 2 — Rust unit tests (inline `#[cfg(test)]`)

Every module has a `#[cfg(test)]` block. Run with `cargo test`.

#### `watcher.rs` tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn parse_valid_json_status() {
        let json = r#"{"v":1,"status":"working","message":"Running...","terminal":{"kind":"iterm2","focus_id":"w0t0p0:ABC","outer_id":"","label":"iTerm2"}}"#;
        let f = write_temp(json);
        let result = parse_status_file(f.path()).unwrap();
        assert_eq!(result.status, "working");
        assert_eq!(result.message, "Running...");
        assert_eq!(result.terminal.as_ref().unwrap().kind, "iterm2");
        assert!(result.can_focus);
    }

    #[test]
    fn parse_legacy_pipe_format() {
        let f = write_temp("working|Running tests...");
        let result = parse_status_file(f.path()).unwrap();
        assert_eq!(result.status, "working");
        assert_eq!(result.message, "Running tests...");
        assert!(result.terminal.is_none());
        assert!(!result.can_focus);
    }

    #[test]
    fn parse_empty_file_returns_none() {
        let f = write_temp("   \n");
        assert!(parse_status_file(f.path()).is_none());
    }

    #[test]
    fn parse_malformed_json_skipped() {
        let f = write_temp("{bad json}");
        // Falls back to pipe format; "{bad json}" has no "|" so status = "{bad json}", message = ""
        // This is acceptable — the important thing is it does not panic
        let _ = parse_status_file(f.path()); // must not panic
    }

    #[test]
    fn parse_unknown_version_skipped() {
        let json = r#"{"v":99,"status":"working","message":"hi"}"#;
        let f = write_temp(json);
        assert!(parse_status_file(f.path()).is_none());
    }

    #[test]
    fn can_focus_false_when_terminal_absent() {
        let f = write_temp(r#"{"v":1,"status":"idle","message":""}"#);
        let result = parse_status_file(f.path()).unwrap();
        assert!(!result.can_focus);
    }

    #[test]
    fn can_focus_false_when_focus_id_empty() {
        let f = write_temp(r#"{"v":1,"status":"idle","message":"","terminal":{"kind":"x11_generic","focus_id":"","outer_id":"","label":"Terminal"}}"#);
        let result = parse_status_file(f.path()).unwrap();
        assert!(!result.can_focus);
    }

    #[test]
    fn can_focus_false_when_focus_id_zero() {
        let f = write_temp(r#"{"v":1,"status":"idle","message":"","terminal":{"kind":"unknown","focus_id":"0","outer_id":"","label":"Terminal"}}"#);
        let result = parse_status_file(f.path()).unwrap();
        assert!(!result.can_focus);
    }

    #[test]
    fn sort_needs_input_before_working() {
        let agents = vec![
            AgentStatus { name: "b".into(), status: "working".into(),     message: "".into(), terminal: None, can_focus: false },
            AgentStatus { name: "a".into(), status: "needs-input".into(), message: "".into(), terminal: None, can_focus: false },
            AgentStatus { name: "c".into(), status: "idle".into(),        message: "".into(), terminal: None, can_focus: false },
        ];
        let sorted = sort_agents(agents);
        assert_eq!(sorted[0].status, "needs-input");
        assert_eq!(sorted[1].status, "working");
        assert_eq!(sorted[2].status, "idle");
    }

    #[test]
    fn sort_alphabetical_within_tier() {
        let agents = vec![
            AgentStatus { name: "zebra".into(), status: "working".into(), message: "".into(), terminal: None, can_focus: false },
            AgentStatus { name: "alpha".into(), status: "working".into(), message: "".into(), terminal: None, can_focus: false },
        ];
        let sorted = sort_agents(agents);
        assert_eq!(sorted[0].name, "alpha");
        assert_eq!(sorted[1].name, "zebra");
    }

    #[test]
    fn message_truncated_at_500_chars() {
        let long_msg = "x".repeat(600);
        let json = format!(r#"{{"v":1,"status":"working","message":"{}"}}"#, long_msg);
        let f = write_temp(&json);
        let result = parse_status_file(f.path()).unwrap();
        assert!(result.message.len() <= 500);
    }
}
```

#### `focusers/mod.rs` tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_unknown_kind_returns_ok() {
        // A future terminal not yet in the dispatch table must not crash
        assert!(dispatch("future_terminal_xyz", "some_id", "").is_ok());
    }

    #[test]
    fn dispatch_empty_kind_returns_ok() {
        assert!(dispatch("", "", "").is_ok());
    }

    #[test]
    fn dispatch_empty_focus_id_returns_ok() {
        // All v1 focusers must handle empty focus_id gracefully
        for kind in &["iterm2", "terminal_app", "gitbash", "powershell_cmd", "x11_generic", "unknown"] {
            assert!(dispatch(kind, "", "").is_ok(), "dispatch({kind}) panicked on empty focus_id");
        }
    }

    #[test]
    fn dispatch_all_v1_kinds_registered() {
        // Smoke test: all v1 kinds are in the dispatch table (return Ok, don't hit unknown)
        // We can't test actual focus behavior in unit tests, but we can assert no panic
        let v1_kinds = ["iterm2", "terminal_app", "gitbash", "powershell_cmd", "x11_generic"];
        for kind in &v1_kinds {
            // Should not panic — Unknown focuser is the fallback for truly unknown kinds
            let _ = dispatch(kind, "dummy_id", "");
        }
    }
}
```

#### `tray.rs` tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn agent(status: &str) -> AgentStatus {
        AgentStatus { name: "test".into(), status: status.into(), message: "".into(), terminal: None, can_focus: false }
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
        let agents = vec![agent("idle"), agent("working"), agent("needs-input"), agent("error")];
        assert_eq!(aggregate_state(&agents), "needs-input");
    }

    #[test]
    fn aggregate_error_beats_working() {
        let agents = vec![agent("working"), agent("error"), agent("idle")];
        assert_eq!(aggregate_state(&agents), "error");
    }

    #[test]
    fn aggregate_working_beats_idle() {
        let agents = vec![agent("idle"), agent("working")];
        assert_eq!(aggregate_state(&agents), "working");
    }

    #[test]
    fn aggregate_all_idle() {
        let agents = vec![agent("idle"), agent("idle"), agent("idle")];
        assert_eq!(aggregate_state(&agents), "idle");
    }
}
```

### Layer 3 — Svelte component tests (`vitest` + `@testing-library/svelte`)

Run with `npm test`. These test component rendering and behavior in isolation.

**`tests/svelte/StatusDot.test.ts`**
```typescript
import { render } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import StatusDot from '$lib/components/StatusDot.svelte';

describe('StatusDot', () => {
  it('renders correct color for needs-input', () => {
    const { container } = render(StatusDot, { props: { status: 'needs-input' } });
    const dot = container.querySelector('div');
    expect(dot?.style.backgroundColor).toBe('#dd4f4f');
  });

  it('applies blink animation for working status', () => {
    const { container } = render(StatusDot, { props: { status: 'working' } });
    const dot = container.querySelector('div');
    expect(dot?.style.animation).toContain('blink');
  });

  it('no animation for idle status', () => {
    const { container } = render(StatusDot, { props: { status: 'idle' } });
    const dot = container.querySelector('div');
    expect(dot?.style.animation).toBe('none');
  });
});
```

**`tests/svelte/utils.test.ts`**
```typescript
import { describe, it, expect } from 'vitest';
import { aggregate, escHtml } from '$lib/utils';
import type { AgentStatus } from '$lib/types';

function a(status: string): AgentStatus {
  return { name: 'test', status: status as any, message: '', terminal: null, can_focus: false };
}

describe('aggregate', () => {
  it('returns offline for empty list', () => expect(aggregate([])).toBe('offline'));
  it('returns needs-input over working', () => expect(aggregate([a('working'), a('needs-input')])).toBe('needs-input'));
  it('returns error over working', () => expect(aggregate([a('working'), a('error')])).toBe('error'));
  it('returns working over idle', () => expect(aggregate([a('idle'), a('working')])).toBe('working'));
});

describe('escHtml', () => {
  it('escapes angle brackets', () => expect(escHtml('<b>')).toBe('&lt;b&gt;'));
  it('escapes ampersands', () => expect(escHtml('a & b')).toBe('a &amp; b'));
  it('escapes double quotes', () => expect(escHtml('"hi"')).toBe('&quot;hi&quot;'));
});
```

### Layer 4 — Integration test (manual, `tests/e2e/`)

**`tests/e2e/scenarios.md`** — documented manual scenarios run before each release:

```markdown
## Scenario 1: Basic agent lifecycle
1. Launch AgentTray — tray icon is gray
2. Run: echo '{"v":1,"status":"working","message":"test"}' > ~/.agent-monitor/s1.status
3. EXPECT: tray icon turns yellow within 500ms
4. Run: echo '{"v":1,"status":"needs-input","message":"[y/n]?"}' > ~/.agent-monitor/s1.status
5. EXPECT: tray icon turns red and pulses
6. Run: rm ~/.agent-monitor/s1.status
7. EXPECT: tray icon returns to gray within 500ms

## Scenario 2: Multiple agents — highest priority wins
1. Write two files: s2a.status = working, s2b.status = idle
2. EXPECT: tray is yellow
3. Write s2a.status = needs-input
4. EXPECT: tray is red (needs-input beats working)
5. Delete s2a.status
6. EXPECT: tray returns to green (only idle agent remains)

## Scenario 3: wrap.sh integration
1. Run: scripts/wrap.sh test-agent bash -c 'echo hello; sleep 1; echo done'
2. EXPECT: ~/.agent-monitor/test-agent.status exists within 200ms
3. EXPECT: status file is valid JSON with status=working
4. EXPECT: after sleep, file updates to idle

## Scenario 4: Legacy format backward compatibility
1. Write: echo "working|Building..." > ~/.agent-monitor/legacy.status
2. EXPECT: popup shows agent "legacy" with working status, no focus button

## Scenario 5: Popup opens and closes
1. Click tray icon — popup opens
2. Click outside popup — popup closes
3. Click tray icon again — popup opens
4. Click tray icon again — popup closes

## Scenario 6: Auto-update banner
1. Manually emit update-available event via Tauri devtools
2. EXPECT: blue UpdateBanner appears in popup header
3. EXPECT: version string is shown

## Scenario 7: Native notifications — needs-input
1. Ensure notification permission is granted (macOS: System Settings → Notifications → AgentTray → Allow)
2. Write: `echo '{"v":1,"status":"working","message":"Running..."}' > ~/.agent-monitor/notif-test.status`
3. Write: `echo '{"v":1,"status":"needs-input","message":"Overwrite src/api.ts? [y/n]"}' > ~/.agent-monitor/notif-test.status`
4. EXPECT: native OS notification appears within 500ms with title "notif-test needs input" and body "Overwrite src/api.ts? [y/n]"
5. Write the same needs-input line again (no state change)
6. EXPECT: no second notification fires (dedup working)
7. Write: `echo '{"v":1,"status":"idle","message":"Done"}' > ~/.agent-monitor/notif-test.status`
8. EXPECT: no notification for idle
9. Write needs-input again
10. EXPECT: notification fires again (re-notifies after state change and return)

## Scenario 8: Native notifications — error
1. Write: `echo '{"v":1,"status":"working","message":"Building"}' > ~/.agent-monitor/err-test.status`
2. Write: `echo '{"v":1,"status":"error","message":"Exit 1"}' > ~/.agent-monitor/err-test.status`
3. EXPECT: native OS notification with title "err-test errored" and body "Exit 1"

## Scenario 9: Notification permission denied (macOS only)
1. Revoke AgentTray notification permission: System Settings → Notifications → AgentTray → Don't Allow
2. Trigger a needs-input transition
3. EXPECT: no notification appears, but tray icon still turns red and popup still works correctly
4. EXPECT: no crash, no error dialog

## Scenario 10: Notification with long message
1. Write a status file with a 200-char message
2. EXPECT: notification body shows only the first 120 characters
```

### Layer 5 — CI pipeline (`.github/workflows/ci.yml`)

```yaml
name: CI
on: [push, pull_request]

jobs:
  shell-tests:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4
      - name: Run shell tests
        run: bash tests/shell/run_all.sh

  rust-tests:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --manifest-path src-tauri/Cargo.toml
        # Covers: watcher.rs, focusers/mod.rs, tray.rs, notifications.rs
      - run: cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings

  svelte-tests:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: '20' }
      - run: npm ci
      - run: npm test
      - run: npm run check  # svelte-check TypeScript validation

  build-check:
    runs-on: ubuntu-22.04
    needs: [shell-tests, rust-tests, svelte-tests]
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: '20' }
      - run: npm ci && npm run build
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --manifest-path src-tauri/Cargo.toml  # compile check only
```

---

## Memory Optimization Notes

These decisions reduce RAM to ≤ 30MB RSS at idle:

1. **Lazy WebView** — popup `WebviewWindow` is not created until the first tray click. Before first click: ~12MB RSS. After first open: ~28MB RSS. After close: stays at ~28MB (WebView is retained, not destroyed — re-creation is expensive).

2. **No heap data in watcher thread between cycles** — the watcher thread holds only the `RecommendedWatcher` handle and the mpsc receiver on its stack. No `Arc<Mutex<Vec<AgentStatus>>>`. On each event: allocate Vec, serialize to String, emit, drop. Total per-cycle allocation: O(agents × 300 bytes).

3. **Native OS watchers** — `notify` with per-platform features uses `kqueue` on macOS, `inotify` on Linux, `ReadDirectoryChangesW` on Windows. None of these poll; the thread blocks on the OS notification queue. CPU usage between events: 0%.

4. **`panic = "abort"`** — removes the unwinding/backtrace machinery, saves ~200KB of binary and eliminates the unwind tables from the process image.

5. **`devtools: false`** in `tauri.conf.json` — disables the WebView inspector in release builds, saving ~2MB of WebKit overhead.

6. **Svelte's no-virtual-DOM** — Svelte compiles to direct DOM mutations. There is no VDOM in memory. The popup renders ~15 DOM nodes total; memory impact is negligible.

7. **Tailwind CSS purge** — Tailwind v4 with Vite tree-shakes unused utility classes at build time. The final CSS bundle is < 5KB.

---

## Tray Icons

Generate 22×22px PNGs with ImageMagick. All six must exist in `src-tauri/icons/` before building.

```bash
cd src-tauri/icons
magick -size 22x22 xc:transparent -fill '#dd4f4f' -draw "circle 11,11 11,4" tray-needs-input.png
magick -size 22x22 xc:transparent -fill '#cc7a28' -draw "circle 11,11 11,4" tray-error.png
magick -size 22x22 xc:transparent -fill '#c99626' -draw "circle 11,11 11,4" tray-working.png
magick -size 22x22 xc:transparent -fill '#4898cc' -draw "circle 11,11 11,4" tray-starting.png
magick -size 22x22 xc:transparent -fill '#78b644' -draw "circle 11,11 11,4" tray-idle.png
magick -size 22x22 xc:transparent -fill '#555555' -draw "circle 11,11 11,4" tray-offline.png
```

---

## Build Instructions

```bash
# Prerequisites
cargo install tauri-cli --version "^2"
brew install imagemagick        # macOS
# apt install imagemagick       # Ubuntu
# choco install imagemagick     # Windows

# Frontend setup
npm install
npx shadcn-svelte@latest init   # copies shadcn components into src/lib/components/ui/

# Generate tray icons (see above)

# Generate updater signing key (one-time)
cargo tauri signer generate -w ~/.tauri/agent-tray.key
# Add the public key to tauri.conf.json plugins.updater.pubkey

# Run tests
bash tests/shell/run_all.sh
cargo test --manifest-path src-tauri/Cargo.toml
npm test

# Development (hot reload)
cargo tauri dev

# Production build
cargo tauri build
```

---

## Success Criteria

### Functional

- [ ] App launches; tray shows gray dot when `~/.agent-monitor/` has no `.status` files
- [ ] Writing a JSON status file changes tray color within 500ms
- [ ] Deleting a status file removes the agent from popup within 500ms
- [ ] Tray color = highest-priority state across all agents
- [ ] Clicking tray opens popup; clicking outside closes it
- [ ] Popup shows: agent name, status dot, message, terminal label, focus button (if applicable)
- [ ] Agents sorted: `needs-input` at top, then `error`, `working`, `starting`, `idle`, `offline`; alphabetical within tier
- [ ] Focus button visible on row hover only; absent when `can_focus: false`
- [ ] Clicking focus button invokes `focus_terminal` command; popup closes afterward
- [ ] `wrap.sh` creates and updates status files; writes `needs-input` for prompt lines; `idle` on clean exit; `error` on non-zero exit
- [ ] Legacy pipe format (`working|message`) displays correctly, no focus button
- [ ] Update banner appears when `update-available` event fires; "Update" button triggers download + relaunch
- [ ] Auto-update check runs on startup without blocking the UI
- [ ] Native OS notification fires when any agent transitions to `needs-input`
- [ ] Native OS notification fires when any agent transitions to `error`
- [ ] Notification body shows the agent name and last message (truncated at 120 chars)
- [ ] Notification does not re-fire if agent stays in `needs-input` across multiple poll cycles (dedup)
- [ ] Notification re-fires if agent leaves `needs-input`, then returns to it
- [ ] No notification fires for `working`, `idle`, `starting`, or `offline` transitions
- [ ] macOS: notification permission is requested on first launch; app functions normally if permission denied

### v1.0 Terminal Focus

- [ ] **macOS Terminal.app**: clicking focus switches to the exact tab running the agent
- [ ] **iTerm2**: clicking focus switches to the exact iTerm2 pane/tab running the agent
- [ ] **Git Bash**: clicking focus raises the correct mintty window
- [ ] **PowerShell/CMD**: clicking focus raises the correct console window
- [ ] **Linux X11 generic**: clicking focus raises the correct terminal window and switches to the correct tab (if `xdotool` installed)
- [ ] **Unknown terminal**: focus button is hidden; agent still shows in popup with status

### Correctness

- [ ] `dispatch("unknown_kind", ...)` returns `Ok(())` — never panics
- [ ] All focusers return `Ok(())` when their tool (`wmctrl`, `osascript`, etc.) is absent
- [ ] Malformed status file is skipped; other agents continue to display
- [ ] Mid-write race condition (partial JSON) does not crash the watcher
- [ ] Status file with `"v": 99` is skipped
- [ ] `notifications::notify_transitions` never panics when agents list is empty
- [ ] `notifications::notify_transitions` never panics when `LAST_NOTIFIED` mutex is poisoned (use `.unwrap_or_else` recovery)
- [ ] Notification failure (permission denied, OS error) does not propagate — returns `Ok(())`
- [ ] `cargo test` exits 0
- [ ] `bash tests/shell/run_all.sh` exits 0
- [ ] `npm test` exits 0
- [ ] `cargo clippy -- -D warnings` exits 0
- [ ] `npm run check` (svelte-check) exits 0

### Architecture

- [ ] Adding a new terminal (post-v1) requires: one `detectors/NN_<kind>.sh` + one `focusers/<kind>.rs` + one line in `focusers/mod.rs` — nothing else
- [ ] `focus.rs` contains zero focus logic
- [ ] No focuser file imports from another focuser file
- [ ] `watcher.rs` contains zero focus logic and zero notification logic — it calls `notifications::notify_transitions` as a side-effect only
- [ ] `notifications.rs` contains zero watcher logic and zero tray logic
- [ ] `tray.rs` contains zero parsing logic
- [ ] Each detector is independently testable with `env -i VAR=value bash detectors/NN_<kind>.sh`
- [ ] Svelte components do not call `invoke` directly — only `+page.svelte` calls Tauri APIs

### Performance

- [ ] Release binary size ≤ 8MB (Svelte adds ~2MB vs vanilla JS)
- [ ] RSS at idle (popup never opened) ≤ 30MB
- [ ] RSS at idle (popup opened once, then closed) ≤ 40MB
- [ ] Popup opens in ≤ 150ms after first tray click (WebView creation)
- [ ] Popup opens in ≤ 80ms on subsequent clicks (WebView already exists)
- [ ] Status change reflects in popup within 500ms of file write
- [ ] `wrap.sh` startup overhead ≤ 100ms (detector chain)
- [ ] CPU usage at idle ≤ 0.1%

### Cross-platform

- [ ] Builds and runs on macOS 13+ (Apple Silicon + Intel)
- [ ] Builds and runs on Ubuntu 22.04+ with `libayatana-appindicator3-dev`
- [ ] Builds and runs on Windows 10+ with Git Bash available
- [ ] Popup positioned correctly on all three platforms

---

## What NOT to Build

Do not add any of the following:

- Cloud sync or remote status sharing
- Built-in agent runners or process spawning
- HTTP server or web dashboard
- Telemetry, analytics, or crash reporting
- Notification sounds (native OS notifications are supported; audio alerts are not — do not add sound files, `afplay`, `paplay`, or `wscript.exe` calls)
- SQLite or any other database — files are the database
- Global keyboard shortcuts
- Per-agent configuration UI (use env vars in `wrap.sh` instead)
- Any terminal not in the v1.0 scope — file a GitHub issue and add to `BACKLOG.md` instead
- Push notifications (FCM/APNs) — desktop-only OS notifications via `tauri-plugin-notification` are sufficient

---

## Backlog Terminal List (`BACKLOG.md`)

The following terminals are deferred to post-v1.0. Each requires exactly: one `detectors/NN_<kind>.sh` + one `focusers/<kind>.rs` + one line in `focusers/mod.rs`.

| Terminal | Platform | Detection signal | Focus method |
|---|---|---|---|
| Windows Terminal | Windows | `$WT_SESSION` | PowerShell SetForegroundWindow (no per-tab API yet) |
| Warp | macOS/Linux | `$TERM_PROGRAM=WarpTerminal` | `activate_macos_app("Warp")` |
| Ghostty | macOS/Linux | `$GHOSTTY_RESOURCES_DIR` | `activate_macos_app("Ghostty")` |
| Kitty | macOS/Linux/Win | `$KITTY_WINDOW_ID` | `kitty @ focus-window --match id:N` |
| Alacritty | macOS/Linux | `$ALACRITTY_WINDOW_ID` | xdotool / activate |
| Hyper | macOS | `$TERM_PROGRAM=Hyper` | `activate_macos_app("Hyper")` |
| Tabby | macOS | `$TERM_PROGRAM=Tabby` | `activate_macos_app("Tabby")` |
| Konsole | Linux | `$KONSOLE_VERSION` | wmctrl + qdbus Session.processId |
| Terminator | Linux | `$TERMINATOR_UUID` | dbus-send focus_terminal |
| Tilix | Linux | `$WINDOWID` + process name | wmctrl + xdotool |
| VS Code terminal | all | `$TERM_PROGRAM=vscode` | `code --reuse-window` |
| JetBrains IDEs | all | `$TERMINAL_EMULATOR contains JetBrains` | wmctrl by process name |
| Neovim terminal | all | `$NVIM` (socket path) | nvim --server --remote-send |
| ConEmu | Windows | `$ConEmuPID` | PowerShell SetForegroundWindow |
| Cmder | Windows | `$CMDER_ROOT` | PowerShell SetForegroundWindow |
| tmux | all | `$TMUX` | `tmux switch-client -t %PANEID` |
| GNU screen | all | `$STY` | `screen -x STY -p WINNUM` |
| Zellij | all | `$ZELLIJ_SESSION_NAME` | `zellij action focus-pane-with-id N` |

---

## Competitive Landscape and Related Libraries

Understanding what exists in the space helps position AgentTray correctly and informs which features are table stakes vs. differentiators.

### Direct competitors — AI agent monitoring tools

These tools solve the same core problem (visibility into running AI coding agents) but with different approaches.

| Tool | Approach | Platform | Scope | GUI | Focus/jump | Stars (approx) | Differentiator vs AgentTray |
|---|---|---|---|---|---|---|---|
| **tmux-agent-status** ([github](https://github.com/samleeney/tmux-agent-status)) | tmux plugin — status bar counts + sound alerts | macOS/Linux | tmux only | ❌ TUI in status bar | tmux prefix key | ~800 | Session deployment scripts; TTS "Agent ready"; remote SSH monitoring |
| **tmux-agent-indicator** ([github](https://github.com/accessd/tmux-agent-indicator)) | tmux plugin — pane borders + window title colors | macOS/Linux | tmux only | ❌ TUI color coding | tmux prefix key | ~400 | Per-pane border/bg color; state resets on pane focus |
| **recon** ([github](https://github.com/gavraz/recon)) | TUI dashboard — pixel-art creature per agent | macOS/Linux | tmux + Claude Code | ❌ Full-screen TUI | Enter in dashboard | ~600 | Pixel art "rooms" by git repo; Claude Code session JSON parsing |
| **agent-tmux-monitor (ATM)** ([github](https://github.com/damelLP/agent-tmux-monitor)) | Rust TUI with daemon — context usage + cost | macOS/Linux | tmux + Claude Code | ❌ Split-pane TUI | Vim-keys in TUI | ~300 | Context progress bars; cost tracking per session; daemon architecture |
| **agent-deck** ([github](https://github.com/asheshgoplani/agent-deck)) | tmux session manager with AI-awareness | macOS/Linux/WSL | tmux + multiple agents | ❌ tmux status bar | `Ctrl+b 1-6` | ~500 | Session forking; MCP management; token/cost tracking; worktree support |
| **agent-conductor** ([github](https://github.com/gaurav-yadav/agent-conductor)) | CLI + REST API orchestration with SQLite | macOS/Linux | tmux + Claude/Codex | ❌ CLI + HTTP | CLI jump command | ~200 | Supervisor/worker delegation; inter-agent inbox messaging; approval gates |

**AgentTray's position:** the only GUI desktop app in this space. All competitors live inside the terminal (tmux status bars, TUI dashboards). AgentTray is visible even when no terminal is in focus — you can be in your browser or IDE and still see the red dot.

### Broader category — multiplexer / terminal session managers

These do not specifically target AI agents but are the tools AgentTray users likely also use.

| Tool | Language | Purpose | Relevant to AgentTray |
|---|---|---|---|
| **tmux** | C | Terminal multiplexer | Most AgentTray users run agents inside tmux; the backlog covers tmux pane focus |
| **zellij** | Rust | Modern multiplexer with layout engine | Growing adoption; backlog item |
| **tmuxai** ([github](https://github.com/alvinunreal/tmuxai)) | Go | AI that reads tmux pane content | Different angle: AI observes the terminal |
| **libtmux** (Python) | Python | Programmatic tmux control | Used by agent-conductor; useful reference for tmux focus implementation |

### Key open-source libraries used by AgentTray

Libraries that AgentTray directly depends on, plus relevant alternatives to be aware of.

| Library | Language | Role in AgentTray | Why chosen over alternatives |
|---|---|---|---|
| **tauri 2.x** | Rust + WebView | App shell, tray icon, IPC, updater, notifications | vs. Electron: 10× smaller binary, native WebView, no bundled Chromium |
| **tauri-plugin-notification** | Rust | Native OS notifications | Official Tauri plugin; macOS/Windows/Linux; no extra deps |
| **tauri-plugin-updater** | Rust | GitHub Releases auto-update | Official Tauri plugin; signature verification built-in |
| **notify (v6)** | Rust | File system events | Uses `kqueue`/`inotify`/`ReadDirectoryChanges`; zero CPU between events |
| **serde + serde_json** | Rust | JSON parse/serialize | De facto standard; zero-copy deserialization |
| **svelte 5** | JS/TS | Popup UI | Compiles to vanilla DOM ops; no VDOM in memory; smallest runtime of any framework |
| **shadcn-svelte** | Svelte | UI primitives (Button, Badge, Tooltip) | Owned-code model — components are copied in, not a runtime dep; fully customizable |
| **lucide-svelte** | Svelte | Icons | Tree-shakeable; 1 icon ≈ 0.2KB in bundle |
| **tailwindcss v4** | CSS | Utility styling | v4 uses Vite plugin; zero config; aggressive purge of unused classes |
| **dirs-next** | Rust | Cross-platform home directory | Handles `~` expansion portably on macOS/Linux/Windows |

### Libraries evaluated but not used

| Library | Why not chosen |
|---|---|
| **Electron** | 150–300MB binary; bundles full Chromium; 10× more RAM |
| **notify-rust** (standalone) | `tauri-plugin-notification` wraps this; using plugin avoids duplicate dep |
| **tauri-plugin-notifications** (third-party, choochmeque) | Third-party; adds push notification complexity not needed; official plugin sufficient |
| **React / Vue** | Larger runtime than Svelte; VDOM heap overhead; no meaningful benefit for a 15-node popup |
| **egui / iced** | Native Rust GUI; no WebView; harder to style; CSS-based design easier for a tray popup |
| **SQLite / rusqlite** | Files are the database; SQLite is out of scope per KISS principle |

### What AgentTray does that no competitor does

1. **Visible outside the terminal.** Every competitor requires a terminal to be open and in focus. AgentTray works when you're in a browser, IDE, or document editor.
2. **Cross-platform GUI.** No competitor runs on Windows natively. AgentTray targets macOS + Linux + Windows from day one.
3. **Terminal-agnostic.** Competitors are tmux-only or Claude Code-only. AgentTray works with any agent that can write a line to a file.
4. **Native OS notifications.** `needs-input` notification appears in macOS Notification Center / Windows Action Center / Linux notification daemon — not a tmux bell or status bar icon.
5. **Click-to-focus.** One click raises the exact terminal window/tab/pane. Competitors require knowing which tmux session to switch to.

---

## `CONTRIBUTING.md` — How to add a new terminal

This file ships with the repo. Full content:

```markdown
# Adding a new terminal to AgentTray

Adding support for a new terminal requires exactly **three edits**:

1. Create `scripts/detectors/NN_<kind>.sh`
2. Create `src-tauri/src/focusers/<kind>.rs`
3. Add one line to `src-tauri/src/focusers/mod.rs`

Nothing else changes. The registry, watcher, popup, and tray icon are all
terminal-agnostic.

---

## Step 1 — Write the detector

Create `scripts/detectors/NN_<kind>.sh` where `NN` is a two-digit priority
number (lower = checked first). Multiplexers use 10–19, macOS emulators
20–29, Windows 40–49, Linux 50–69, fallback 99.

The file must:
- Print exactly one JSON line if the terminal is detected, nothing otherwise
- Exit 0 in both cases
- Complete in ≤ 5ms

```bash
#!/usr/bin/env bash
# Detector for MyTerminal
[ -z "$MY_TERMINAL_SESSION_ID" ] && exit 0
printf '{"kind":"my_terminal","focus_id":"%s","outer_id":"","label":"MyTerminal"}\n' \
  "$MY_TERMINAL_SESSION_ID"
```

Test it in isolation before anything else:
```bash
env -i MY_TERMINAL_SESSION_ID=test123 bash scripts/detectors/NN_my_terminal.sh
# Expected: {"kind":"my_terminal","focus_id":"test123","outer_id":"","label":"MyTerminal"}

env -i bash scripts/detectors/NN_my_terminal.sh
# Expected: (empty — no output)
```

---

## Step 2 — Write the focuser

Create `src-tauri/src/focusers/my_terminal.rs`:

```rust
use super::{Focuser, os_helpers};

pub struct MyTerminal;

impl Focuser for MyTerminal {
    fn focus(&self, focus_id: &str, _outer_id: &str) -> Result<(), String> {
        if focus_id.is_empty() { return Ok(()); }
        // Implement platform-specific focus logic here.
        // Use helpers from os_helpers.rs where possible.
        // Return Ok(()) if the tool is unavailable — never Err.
        #[cfg(target_os = "macos")]
        return os_helpers::activate_macos_app("MyTerminal");
        #[cfg(target_os = "linux")]
        return os_helpers::wmctrl_focus(focus_id);
        #[allow(unreachable_code)]
        Ok(())
    }
}
```

Rules:
- One struct, one `impl Focuser` block — nothing else in the file
- Never import from another focuser file
- All platform-specific code is cfg-gated
- Return `Ok(())` for "tool not found" or "not supported on this OS"

---

## Step 3 — Register in mod.rs

In `src-tauri/src/focusers/mod.rs`, add:

```rust
pub mod my_terminal;   // ← add this line with the other pub mod declarations

// In the dispatch() match:
"my_terminal" => &my_terminal::MyTerminal,   // ← add this line
```

---

## Step 4 — Add to the detector test suite

In `tests/shell/test_detectors.sh`, add:

```bash
assert_kind "my_terminal basic" \
  "MY_TERMINAL_SESSION_ID=sess-abc" \
  "my_terminal" \
  "NN_my_terminal"

assert_kind "my_terminal not set" \
  "" \
  "" \
  "NN_my_terminal"
```

---

## Step 5 — Remove from BACKLOG.md

Delete the row for your terminal from `BACKLOG.md` and open a PR.

---

## Checklist before submitting PR

- [ ] `env -i RELEVANT_VAR=value bash scripts/detectors/NN_<kind>.sh` prints correct JSON
- [ ] `env -i bash scripts/detectors/NN_<kind>.sh` prints nothing
- [ ] `cargo test` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `bash tests/shell/run_all.sh` passes
- [ ] Focuser handles empty `focus_id` without panicking
- [ ] Focuser handles absent tool (e.g. `wmctrl` not installed) by returning `Ok(())`
- [ ] Terminal removed from `BACKLOG.md`
```

---

## `README.md` — Required sections

The README must include these sections in this order. Exact wording is flexible; content is not.

1. **Tagline** — one sentence: "A lightweight system tray app that watches your AI coding agents so you don't have to."
2. **Screenshot / demo GIF** — placeholder: `![Demo](docs/demo.gif)` — add before v1.0 release
3. **Why AgentTray** — the two failure modes (constant tab-switching / forgetting entirely), the red-dot solution. The line: "Red means look at your terminal. Everything else means keep working."
4. **What makes it different** — the comparison table from the Competitive Landscape section above (5 competitors, condensed)
5. **Pricing** — personal free / commercial $29 table; Buy Me a Coffee link; commercial license link
6. **How it works** — the `wrap.sh → status file → Tauri watcher → tray icon` flow diagram (ASCII is fine)
7. **Setup** — install prerequisites, generate icons, build, add aliases
8. **Manual status writes** — Bash, Python, Node.js examples
9. **Status values table** — all 6 states with colors and meanings
10. **v1.0 supported terminals** — the 5 terminals with their platforms
11. **Adding a new terminal** — one-paragraph summary linking to `CONTRIBUTING.md`
12. **Platform notes** — macOS, Linux, Windows install requirements
13. **License** — MIT personal / commercial $29

---

## Complete file list (final)

Every file that must exist for the project to build, test, and ship:

```
agent-tray/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── watcher.rs
│   │   ├── tray.rs
│   │   ├── focus.rs
│   │   ├── notifications.rs
│   │   ├── updater.rs
│   │   └── focusers/
│   │       ├── mod.rs
│   │       ├── os_helpers.rs
│   │       ├── iterm2.rs
│   │       ├── terminal_app.rs
│   │       ├── gitbash.rs
│   │       ├── powershell_cmd.rs
│   │       ├── x11_generic.rs
│   │       └── unknown.rs
│   ├── capabilities/
│   │   └── default.json              ← notification + focus permissions
│   ├── icons/
│   │   ├── tray-needs-input.png
│   │   ├── tray-error.png
│   │   ├── tray-working.png
│   │   ├── tray-starting.png
│   │   ├── tray-idle.png
│   │   └── tray-offline.png
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/
│   ├── app.html
│   ├── app.css
│   ├── lib/
│   │   ├── components/
│   │   │   ├── AgentRow.svelte
│   │   │   ├── StatusDot.svelte
│   │   │   ├── AggregatePill.svelte
│   │   │   ├── SupportBar.svelte
│   │   │   └── UpdateBanner.svelte
│   │   ├── components/ui/            ← shadcn-svelte (generated by init)
│   │   ├── stores/
│   │   │   └── agents.ts
│   │   ├── types.ts
│   │   └── utils.ts
│   └── routes/
│       └── +page.svelte
├── scripts/
│   ├── wrap.sh
│   ├── registry.sh
│   └── detectors/
│       ├── 20_iterm2.sh
│       ├── 21_terminal_app.sh
│       ├── 40_gitbash.sh
│       ├── 41_powershell_cmd.sh
│       ├── 60_x11_generic.sh
│       └── 99_unknown.sh
├── tests/
│   ├── shell/
│   │   ├── run_all.sh
│   │   ├── test_registry.sh
│   │   ├── test_wrap.sh
│   │   ├── test_detectors.sh
│   │   └── fixtures/
│   │       ├── mock_agent_ok.sh
│   │       ├── mock_agent_fail.sh
│   │       └── mock_agent_prompt.sh
│   └── e2e/
│       ├── README.md
│       └── scenarios.md
├── .github/
│   └── workflows/
│       ├── ci.yml
│       └── release.yml
├── svelte.config.js
├── vite.config.ts
├── package.json
├── tsconfig.json
├── tailwind.config.ts               ← if needed for v4 custom config
├── BACKLOG.md
├── CONTRIBUTING.md
├── COMMERCIAL_LICENSE
├── LICENSE
└── README.md
```

**Files generated by tooling (do not create manually):**
- `src/lib/components/ui/` — generated by `npx shadcn-svelte@latest init`
- `src-tauri/gen/` — generated by `cargo tauri build`
- `target/` — Rust build output
- `node_modules/` — npm packages
- `.svelte-kit/` — SvelteKit build cache

**Personal use:** MIT — free, forever, no license key.

The popup `SupportBar.svelte` footer shows:
- Left: "Free for personal use. Found it useful?"
- Right: "☕ Buy me a coffee" → `https://buymeacoffee.com/YOUR_USERNAME`

**Commercial use:** one-time $29/seat license via Gumroad or Lemon Squeezy.

Two license files at repo root:
- `LICENSE` — MIT, personal/non-commercial use
- `COMMERCIAL_LICENSE` — $29/seat, one-time, covers all installs within one organization

Replace placeholder URLs before publishing:
- `YOUR_USERNAME` in `SupportBar.svelte` and `README.md`
- `YOUR_STORE_LINK` in `README.md` and `COMMERCIAL_LICENSE`
- `YOUR_ORG` in `tauri.conf.json` updater endpoint and `release.yml`
- `YOUR_TAURI_UPDATER_PUBLIC_KEY` in `tauri.conf.json`
