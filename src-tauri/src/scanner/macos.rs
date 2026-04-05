use std::path::{Path, PathBuf};

use crate::watcher::TerminalInfo;
use super::{known_terminal, ProcInfo, WindowCache};
use super::strategies::CliStrategy;

/// Find CLI processes matching any registered strategy.
pub fn find_cli_processes<'a>(
    strategies: &'a [Box<dyn CliStrategy>],
) -> Vec<(ProcInfo, &'a dyn CliStrategy)> {
    let output = match std::process::Command::new("ps")
        .args(["-eo", "pid,ppid,pcpu,tty,command"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut out = Vec::new();

    for line in stdout.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }

        let cmd_path = parts[4];
        let exe_name = Path::new(cmd_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let strategy = match strategies.iter().find(|s| {
            s.process_names().iter().any(|n| *n == exe_name)
        }) {
            Some(s) => s.as_ref(),
            None => continue,
        };

        let full_cmd = parts[4..].join(" ");
        if strategy.excluded_substrings().iter().any(|exc| full_cmd.contains(exc)) {
            continue;
        }

        let pid: u32 = match parts[0].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let ppid: u32 = parts[1].parse().unwrap_or(0);
        let cpu: f64 = parts[2].parse().unwrap_or(0.0);
        let tty_label = if parts[3] == "??" {
            String::new()
        } else {
            parts[3].to_string()
        };

        // Get CWD via lsof, fall back to PWD from process environment
        let cwd = lsof_cwd(pid)
            .or_else(|| env_cwd(pid))
            .unwrap_or_else(|| PathBuf::from("/"));

        if tty_label.is_empty() {
            continue; // skip non-terminal processes
        }

        out.push((ProcInfo {
            pid,
            ppid,
            cwd,
            tty_label,
            utime: 0,
            stime: 0,
            instant_cpu: Some(cpu),
            window_title: None,
            last_active: None,
        }, strategy));
    }

    out
}

pub fn terminal_info(cache: &mut WindowCache, p: &ProcInfo) -> Option<TerminalInfo> {
    let mut cur = p.ppid;
    let mut term_app = String::new();
    let mut term_pid: u32 = 0;

    for _ in 0..6 {
        if cur <= 1 {
            break;
        }
        let output = std::process::Command::new("ps")
            .args(["-p", &cur.to_string(), "-o", "ppid=,command="])
            .output()
            .ok()?;
        let line = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = line.trim().splitn(2, char::is_whitespace).collect();
        if parts.len() < 2 {
            break;
        }

        let exe = parts[1].split_whitespace().next().unwrap_or("");
        let exe_name = Path::new(exe)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if let Some(label) = known_terminal(exe_name) {
            term_app = label.to_string();
            term_pid = cur;
            break;
        }

        cur = parts[0].trim().parse().unwrap_or(0);
    }

    if term_app.is_empty() {
        term_app = p.tty_label.clone();
    }

    // Use TTY-based AppleScript for precise window title (works without
    // Accessibility permissions), then fall back to System Events.
    let tty = &p.tty_label;
    let window_title = if !term_app.is_empty() && term_app != *tty {
        cache
            .entry(term_pid)
            .or_insert_with(|| {
                tty_window_title(&term_app, tty)
                    .or_else(|| osascript_window_title(term_pid))
            })
            .clone()
    } else {
        None
    };

    Some(TerminalInfo {
        kind: "macos_app".to_string(),
        focus_id: term_app.clone(),
        outer_id: tty.clone(), // TTY for tab-specific focus
        label: term_app,
        window_title,
    })
}

fn lsof_cwd(pid: u32) -> Option<PathBuf> {
    let output = std::process::Command::new("lsof")
        .args(["-p", &pid.to_string(), "-a", "-d", "cwd", "-Fn"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix('n') {
            return Some(PathBuf::from(path));
        }
    }
    None
}

/// Fallback CWD detection: read PWD from process environment via `ps eww`.
fn env_cwd(pid: u32) -> Option<PathBuf> {
    let output = std::process::Command::new("ps")
        .args(["eww", "-p", &pid.to_string(), "-o", "command="])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for part in stdout.split_whitespace() {
        if let Some(pwd) = part.strip_prefix("PWD=") {
            let path = PathBuf::from(pwd);
            if path.is_absolute() {
                return Some(path);
            }
        }
    }
    None
}

/// Get window title using terminal-specific AppleScript keyed by TTY.
/// More reliable than System Events (doesn't need Accessibility permissions)
/// and finds the exact tab, not just the front window.
fn tty_window_title(app_name: &str, tty: &str) -> Option<String> {
    let script = match app_name {
        "iTerm2" => format!(
            r#"tell application "iTerm2"
                repeat with w in windows
                    repeat with t in tabs of w
                        repeat with s in sessions of t
                            if tty of s contains "{}" then
                                return name of w
                            end if
                        end repeat
                    end repeat
                end repeat
            end tell"#,
            tty
        ),
        "Terminal" => format!(
            r#"tell application "Terminal"
                repeat with w in windows
                    repeat with t in tabs of w
                        if tty of t contains "{}" then
                            return name of w
                        end if
                    end repeat
                end repeat
            end tell"#,
            tty
        ),
        _ => return None,
    };
    let output = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .ok()?;
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() { None } else { Some(name) }
}

fn osascript_window_title(term_pid: u32) -> Option<String> {
    let script = format!(
        "tell application \"System Events\" to get name of first window of \
         (first process whose unix id is {})",
        term_pid
    );
    let output = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .ok()?;
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() { None } else { Some(name) }
}
