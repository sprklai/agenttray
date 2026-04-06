use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use crate::notifier::{CompositeNotifier, DesktopNotifier, SystemBeepNotifier};

use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::scanner::Scanner;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInfo {
    pub kind: String,
    pub focus_id: String,
    pub outer_id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    /// Stable identity for dedup, list keying, and notification tracking.
    /// For scanned agents: "scan:<tty_label>". For file-backed: "file:<stem>".
    pub id: String,
    pub name: String,
    pub status: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<TerminalInfo>,
    pub can_focus: bool,
    /// CPU usage percentage (None for file-based agents without CPU data).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<f64>,
    /// Detection source: "hook", "wrap", or "scan".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Which CLI tool: "claude-code", "codex", "gemini".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli: Option<String>,
    /// Original session ID from the CLI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Hook event name (e.g. "Notification", "PreToolUse").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook_event: Option<String>,
    /// Hook matcher/subtype (e.g. "permission_prompt", "Bash").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook_matcher: Option<String>,
}

pub fn status_dir() -> Option<PathBuf> {
    dirs_next::home_dir().map(|h| h.join(".agent-monitor"))
}

pub fn status_priority_num(status: &str) -> u8 {
    match status {
        "needs-input" => 0,
        "error" => 1,
        "working" => 2,
        "starting" => 3,
        "idle" => 4,
        _ => 5, // offline or unknown
    }
}

pub fn parse_status_file(path: &Path) -> Option<AgentStatus> {
    let content = std::fs::read_to_string(path).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }

    let name = path.file_stem()?.to_str()?.to_string();

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
        // Check version — skip unknown versions
        if let Some(v) = val.get("v") {
            if v.as_u64() != Some(1) {
                return None;
            }
        }

        let status = val.get("status")?.as_str()?.to_string();
        let mut message = val
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();
        if message.len() > 500 {
            if let Some((idx, _)) = message.char_indices().nth(500) {
                message.truncate(idx);
            }
        }

        let terminal: Option<TerminalInfo> = val
            .get("terminal")
            .and_then(|t| serde_json::from_value(t.clone()).ok());

        let can_focus = terminal
            .as_ref()
            .map(|t| !t.focus_id.is_empty() && t.focus_id != "0")
            .unwrap_or(false);

        let source = val.get("source").and_then(|s| s.as_str()).map(String::from);
        let cli = val.get("cli").and_then(|s| s.as_str()).map(String::from);
        let session_id = val.get("session_id").and_then(|s| s.as_str()).map(String::from);
        let hook_event = val.get("hook_event").and_then(|s| s.as_str()).map(String::from);
        let hook_matcher = val.get("hook_matcher").and_then(|s| s.as_str()).map(String::from);

        let id = format!("file:{}", name);
        Some(AgentStatus {
            id,
            name,
            status,
            message,
            terminal,
            can_focus,
            cpu: None,
            source,
            cli,
            session_id,
            hook_event,
            hook_matcher,
        })
    } else {
        // Legacy pipe format: "status|message"
        let (status, message) = match trimmed.split_once('|') {
            Some((s, m)) => (s.to_string(), m.to_string()),
            None => (trimmed.to_string(), String::new()),
        };
        let id = format!("file:{}", name);
        Some(AgentStatus {
            id,
            name,
            status,
            message,
            terminal: None,
            can_focus: false,
            cpu: None,
            source: Some("wrap".into()),
            cli: None,
            session_id: None,
            hook_event: None,
            hook_matcher: None,
        })
    }
}

/// How long an offline/error status file can sit untouched before we hide it.
const STALE_TTL: Duration = Duration::from_secs(10 * 60); // 10 minutes

fn read_all(dir: &Path) -> Vec<AgentStatus> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let now = std::time::SystemTime::now();

    let mut agents: Vec<AgentStatus> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("status") {
                return None;
            }

            let agent = parse_status_file(&path)?;

            // Filter stale status files: if the file hasn't been written to
            // in STALE_TTL, the session likely exited without sending
            // SessionEnd.  The scanner will still show live processes.
            if let Ok(meta) = std::fs::metadata(&path) {
                if let Ok(mtime) = meta.modified() {
                    if now.duration_since(mtime).unwrap_or_default() > STALE_TTL {
                        return None;
                    }
                }
            }

            Some(agent)
        })
        .collect();

    sort_agents(&mut agents);
    agents
}

fn sort_agents(agents: &mut [AgentStatus]) {
    agents.sort_by(|a, b| {
        let pa = status_priority_num(&a.status);
        let pb = status_priority_num(&b.status);
        pa.cmp(&pb).then_with(|| a.name.cmp(&b.name))
    });
}

/// Cached latest merged agent list for on-demand popup refresh.
static LATEST_AGENTS: Mutex<Vec<AgentStatus>> = Mutex::new(Vec::new());
static NOTIFIER: LazyLock<CompositeNotifier> = LazyLock::new(|| {
    CompositeNotifier::new(vec![
        Box::new(SystemBeepNotifier),
        Box::new(DesktopNotifier),
    ])
});

fn emit_agents(app: &AppHandle, agents: Vec<AgentStatus>) {
    log::info!("emit_agents: {} agents", agents.len());
    for a in &agents {
        log::debug!("  agent: name={:?} status={:?}", a.name, a.status);
    }
    {
        let mut cache = LATEST_AGENTS.lock().unwrap_or_else(|e| {
            log::warn!("LATEST_AGENTS mutex poisoned, recovering");
            e.into_inner()
        });
        crate::notifier::detect_and_notify(&cache, &agents, &*NOTIFIER, Some(app));
        *cache = agents.clone();
    }
    crate::tray::update_icon(app, &agents);
    let _ = app.emit("agents-updated", &agents);
}

/// Re-emit the last known agent list (used when popup opens).
pub fn emit_latest(app: &AppHandle) {
    let agents = LATEST_AGENTS.lock().unwrap_or_else(|e| {
        log::warn!("LATEST_AGENTS mutex poisoned, recovering");
        e.into_inner()
    }).clone();
    crate::tray::update_icon(app, &agents);
    let _ = app.emit("agents-updated", &agents);
}

/// Tauri command: return the platform-appropriate status directory path.
#[tauri::command]
pub fn get_status_dir() -> String {
    status_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default()
}

/// Tauri command: return cached agents (called by frontend on mount).
#[tauri::command]
pub fn get_agents() -> Vec<AgentStatus> {
    let agents = LATEST_AGENTS.lock().unwrap_or_else(|e| {
        log::warn!("LATEST_AGENTS mutex poisoned, recovering");
        e.into_inner()
    }).clone();
    log::info!("get_agents: returning {} agents", agents.len());
    agents
}

/// Tauri command: install or uninstall AgentTray hooks for a CLI tool.
/// `cli` should be "claude", "codex", "gemini", or "all".
/// Set `uninstall` to true to remove hooks instead.
#[tauri::command]
pub fn install_hooks(cli: String, uninstall: bool) -> Result<String, String> {
    let base_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .and_then(|dir| {
            // In dev mode, scripts are relative to the project root
            // In production, they're bundled as resources
            dir.ancestors()
                .find(|d| d.join("scripts").exists())
                .map(|d| d.to_path_buf())
                .or_else(|| Some(dir))
        })
        .ok_or_else(|| "Could not locate scripts directory".to_string())?;

    // On Windows, prefer PowerShell installer; fall back to bash
    let (program, script) = if cfg!(target_os = "windows") {
        let ps1 = base_dir.join("scripts/hooks/install-hooks.ps1");
        if ps1.exists() {
            ("powershell".to_string(), ps1)
        } else {
            let sh = base_dir.join("scripts/hooks/install-hooks.sh");
            ("bash".to_string(), sh)
        }
    } else {
        let sh = base_dir.join("scripts/hooks/install-hooks.sh");
        ("bash".to_string(), sh)
    };

    if !script.exists() {
        return Err(format!("Hook installer not found at: {}", script.display()));
    }

    let mut cmd = std::process::Command::new(&program);
    if program == "powershell" {
        cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]);
        cmd.arg(&script);
        cmd.args(["-Target", &cli]);
        if uninstall {
            cmd.arg("-Uninstall");
        }
    } else {
        cmd.arg(&script).arg(&cli);
        if uninstall {
            cmd.arg("--uninstall");
        }
    }

    let output = cmd.output().map_err(|e| format!("Failed to run installer: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        log::info!("Hook installation succeeded for '{}': {}", cli, stdout.trim());
        Ok(stdout)
    } else {
        let msg = if stderr.is_empty() { &stdout } else { &stderr };
        log::error!("Hook installation failed for '{}': {}", cli, msg.trim());
        Err(format!("Installation failed: {}", msg.trim()))
    }
}

/// Source priority for dedup: hook beats wrap beats scan (lower = higher priority).
fn source_priority(source: Option<&str>) -> u8 {
    match source {
        Some("hook") => 0,
        Some("wrap") => 1,
        Some("scan") => 2,
        _ => 1, // default to wrap-level for legacy files without source
    }
}

fn read_and_emit_merged(app: &AppHandle, dir: &Path, scanned: &[AgentStatus]) {
    let file_agents = read_all(dir);

    // Dedup file agents: when multiple files share the same focus_id,
    // keep the one with the highest source priority (hook > wrap).
    let mut by_focus: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut keep = vec![true; file_agents.len()];

    for (i, a) in file_agents.iter().enumerate() {
        let fid = a.terminal.as_ref()
            .map(|t| t.focus_id.as_str())
            .unwrap_or("");
        if fid.is_empty() || fid == "0" {
            continue; // no focus_id — always keep
        }
        if let Some(&prev_idx) = by_focus.get(fid) {
            let prev_prio = source_priority(file_agents[prev_idx].source.as_deref());
            let cur_prio = source_priority(a.source.as_deref());
            if cur_prio < prev_prio {
                keep[prev_idx] = false;
                by_focus.insert(fid.to_string(), i);
            } else {
                keep[i] = false;
            }
        } else {
            by_focus.insert(fid.to_string(), i);
        }
    }

    let deduped_files: Vec<AgentStatus> = file_agents.into_iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, a)| a)
        .collect();

    // Collect focus_ids from surviving file agents for scan dedup
    let file_focus_ids: std::collections::HashSet<String> = deduped_files
        .iter()
        .filter_map(|a| a.terminal.as_ref())
        .filter(|t| !t.focus_id.is_empty() && t.focus_id != "0")
        .map(|t| t.focus_id.clone())
        .collect();

    let mut agents = deduped_files;
    for s in scanned {
        let dominated = s
            .terminal
            .as_ref()
            .map(|t| !t.focus_id.is_empty() && file_focus_ids.contains(&t.focus_id))
            .unwrap_or(false);
        if !dominated {
            agents.push(s.clone());
        }
    }

    sort_agents(&mut agents);
    emit_agents(app, agents);
}

pub fn watch(app: AppHandle) {
    let dir = match status_dir() {
        Some(d) => d,
        None => {
            log::error!("Could not determine home directory");
            return;
        }
    };

    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::error!("Failed to create {:?}: {}", dir, e);
        return;
    }

    let mut scanner = Scanner::new();
    let scan_interval = Duration::from_secs(2);
    let mut last_scan = Instant::now();

    // Initial emit with first scan
    let scanned = scanner.scan();
    read_and_emit_merged(&app, &dir, &scanned);

    // Set up file watcher
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
        Ok(w) => w,
        Err(e) => {
            log::error!("Failed to create file watcher: {}", e);
            log::info!("Falling back to scan-only mode");
            loop {
                std::thread::sleep(scan_interval);
                let scanned = scanner.scan();
                read_and_emit_merged(&app, &dir, &scanned);
            }
        }
    };

    if let Err(e) = watcher.watch(&dir, RecursiveMode::NonRecursive) {
        log::error!("Failed to watch {:?}: {}", dir, e);
        return;
    }

    log::info!("Watching {:?} + scanning /proc for live agents", dir);

    let debounce = Duration::from_millis(50);
    let mut last_emit = Instant::now() - debounce;
    let mut latest_scan: Vec<AgentStatus> = scanned;

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(event)) => {
                if matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                ) && last_emit.elapsed() >= debounce
                {
                    read_and_emit_merged(&app, &dir, &latest_scan);
                    last_emit = Instant::now();
                }
            }
            Ok(Err(e)) => log::warn!("Watch error: {}", e),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                log::error!("File watcher channel disconnected");
                break;
            }
        }

        // Periodic process scan
        if last_scan.elapsed() >= scan_interval {
            latest_scan = scanner.scan();
            read_and_emit_merged(&app, &dir, &latest_scan);
            last_scan = Instant::now();
            last_emit = Instant::now();
        }
    }
}

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

    fn test_agent(id: &str, name: &str, status: &str) -> AgentStatus {
        AgentStatus {
            id: id.into(), name: name.into(), status: status.into(), message: "".into(),
            terminal: None, can_focus: false, cpu: None,
            source: None, cli: None, session_id: None, hook_event: None, hook_matcher: None,
        }
    }

    fn test_agent_with_terminal(id: &str, name: &str, status: &str, focus_id: &str, label: &str, source: Option<&str>) -> AgentStatus {
        AgentStatus {
            id: id.into(), name: name.into(), status: status.into(), message: "".into(),
            terminal: Some(TerminalInfo {
                kind: "x11_generic".into(), focus_id: focus_id.into(),
                outer_id: "".into(), label: label.into(), window_title: None,
            }),
            can_focus: !focus_id.is_empty(),
            cpu: None,
            source: source.map(|s| s.into()),
            cli: None, session_id: None, hook_event: None, hook_matcher: None,
        }
    }

    #[test]
    fn sort_needs_input_before_working() {
        let mut agents = vec![
            test_agent("t:b", "b", "working"),
            test_agent("t:a", "a", "needs-input"),
            test_agent("t:c", "c", "idle"),
        ];
        sort_agents(&mut agents);
        assert_eq!(agents[0].status, "needs-input");
        assert_eq!(agents[1].status, "working");
        assert_eq!(agents[2].status, "idle");
    }

    #[test]
    fn dedup_scanned_when_file_agent_has_same_focus_id() {
        let file_agent = test_agent_with_terminal("file:myagent", "myagent", "working", "0x1234", "Kitty", Some("wrap"));
        let scanned_dup = test_agent_with_terminal("scan:pts/1", "Project · Kitty · pts/1", "idle", "0x1234", "Kitty", Some("scan"));
        let scanned_unique = test_agent_with_terminal("scan:pts/2", "Other · Alacritty", "working", "0x5678", "Alacritty", Some("scan"));

        let file_agents = vec![file_agent.clone()];
        let scanned = vec![scanned_dup, scanned_unique.clone()];

        let file_focus_ids: std::collections::HashSet<String> = file_agents
            .iter()
            .filter_map(|a| a.terminal.as_ref())
            .filter(|t| !t.focus_id.is_empty())
            .map(|t| t.focus_id.clone())
            .collect();

        let mut agents = file_agents;
        for s in &scanned {
            let dominated = s
                .terminal
                .as_ref()
                .map(|t| !t.focus_id.is_empty() && file_focus_ids.contains(&t.focus_id))
                .unwrap_or(false);
            if !dominated {
                agents.push(s.clone());
            }
        }

        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].name, "myagent");
        assert_eq!(agents[1].name, "Other · Alacritty");
    }

    #[test]
    fn dedup_hook_beats_wrap_same_focus_id() {
        // hook source should beat wrap source when they share a focus_id
        assert!(source_priority(Some("hook")) < source_priority(Some("wrap")));
        assert!(source_priority(Some("wrap")) < source_priority(Some("scan")));
    }

    #[test]
    fn parse_hook_source_fields() {
        let json = r#"{"v":1,"status":"needs-input","message":"Waiting for permission","source":"hook","cli":"claude-code","session_id":"abc123","hook_event":"Notification","hook_matcher":"permission_prompt","terminal":{"kind":"x11_generic","focus_id":"12345:999","outer_id":"","label":"Kitty"}}"#;
        let f = write_temp(json);
        let result = parse_status_file(f.path()).unwrap();
        assert_eq!(result.source.as_deref(), Some("hook"));
        assert_eq!(result.cli.as_deref(), Some("claude-code"));
        assert_eq!(result.session_id.as_deref(), Some("abc123"));
        assert_eq!(result.hook_event.as_deref(), Some("Notification"));
        assert_eq!(result.hook_matcher.as_deref(), Some("permission_prompt"));
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
