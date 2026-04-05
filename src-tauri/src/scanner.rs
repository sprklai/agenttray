use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use crate::watcher::{AgentStatus, TerminalInfo};

struct CpuSnapshot {
    total_ticks: u64,
    when: Instant,
    /// Last time this process had significant CPU activity.
    last_active: Option<Instant>,
}

/// Scans for live Claude CLI processes across platforms.
pub struct Scanner {
    prev: HashMap<u32, CpuSnapshot>,
    #[cfg(target_os = "linux")]
    window_cache: HashMap<u32, Option<String>>,
}

/// Seconds after last activity before we consider the agent "idle"
/// rather than "needs-input". If Claude was recently working and is
/// now quiet, it's likely waiting for user approval/input.
const NEEDS_INPUT_WINDOW_SECS: u64 = 120;

struct ProcInfo {
    pid: u32,
    ppid: u32,
    cwd: PathBuf,
    tty_label: String,
    utime: u64,
    stime: u64,
    /// Pre-computed CPU% (macOS `ps` gives this directly).
    instant_cpu: Option<f64>,
}

impl Scanner {
    pub fn new() -> Self {
        Self {
            prev: HashMap::new(),
            #[cfg(target_os = "linux")]
            window_cache: HashMap::new(),
        }
    }

    pub fn scan(&mut self) -> Vec<AgentStatus> {
        let procs = find_cli_processes();
        let mut agents = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for p in &procs {
            seen.insert(p.pid);

            let cpu_pct = self.cpu_pct(p);
            let is_active = cpu_pct > 2.0;

            // Carry forward last_active from previous snapshot
            let prev_last_active = self.prev.get(&p.pid).and_then(|s| s.last_active);
            let last_active = if is_active {
                Some(Instant::now())
            } else {
                prev_last_active
            };

            // Determine status:
            //  - High CPU → working
            //  - Low CPU, was recently active → needs-input (waiting for approval)
            //  - Low CPU, idle for a while → idle
            let status = if is_active {
                "working"
            } else if let Some(t) = last_active {
                if t.elapsed().as_secs() < NEEDS_INPUT_WINDOW_SECS {
                    "needs-input"
                } else {
                    "idle"
                }
            } else {
                "idle"
            };

            let project = p
                .cwd
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let name = if p.tty_label.is_empty() {
                project.to_string()
            } else {
                format!("{} · {}", project, p.tty_label)
            };

            let message = match status {
                "working" => format!("Active ({:.0}% CPU)", cpu_pct),
                "needs-input" => "Waiting for input".to_string(),
                _ => p.cwd.display().to_string(),
            };

            let terminal = self.terminal_info(p);
            let can_focus = terminal
                .as_ref()
                .map(|t| !t.focus_id.is_empty())
                .unwrap_or(false);

            agents.push(AgentStatus {
                name,
                status: status.to_string(),
                message,
                terminal,
                can_focus,
            });

            self.prev.insert(
                p.pid,
                CpuSnapshot {
                    total_ticks: p.utime + p.stime,
                    when: Instant::now(),
                    last_active,
                },
            );
        }

        self.prev.retain(|pid, _| seen.contains(pid));
        agents
    }

    fn cpu_pct(&self, p: &ProcInfo) -> f64 {
        if let Some(instant) = p.instant_cpu {
            return instant;
        }
        let Some(prev) = self.prev.get(&p.pid) else {
            return 0.0;
        };
        let dt = prev.when.elapsed().as_secs_f64();
        if dt < 0.01 {
            return 0.0;
        }
        let ticks_now = p.utime + p.stime;
        let delta = ticks_now.saturating_sub(prev.total_ticks) as f64;
        (delta / 100.0 / dt) * 100.0 // CLK_TCK = 100 on Linux
    }
}

// ===========================================================================
// Known terminal emulators (cross-platform)
// ===========================================================================

fn known_terminal(exe_name: &str) -> Option<&'static str> {
    match exe_name {
        // Linux
        "warp" | "warp-terminal" => Some("Warp"),
        "kitty" => Some("Kitty"),
        "alacritty" => Some("Alacritty"),
        "gnome-terminal-server" | "gnome-terminal" => Some("GNOME Terminal"),
        "konsole" => Some("Konsole"),
        "xterm" => Some("XTerm"),
        "tilix" => Some("Tilix"),
        "terminator" => Some("Terminator"),
        "xfce4-terminal" => Some("XFCE Terminal"),
        "mate-terminal" => Some("MATE Terminal"),
        "lxterminal" => Some("LXTerminal"),
        "foot" => Some("Foot"),
        "st" => Some("st"),
        "urxvt" | "urxvtd" => Some("URxvt"),
        "ghostty" => Some("Ghostty"),
        // macOS
        "Terminal" => Some("Terminal"),
        "iTerm2" => Some("iTerm2"),
        // Windows
        "WindowsTerminal.exe" | "WindowsTerminal" => Some("Windows Terminal"),
        "cmd.exe" | "cmd" => Some("CMD"),
        "powershell.exe" | "powershell" | "pwsh.exe" | "pwsh" => Some("PowerShell"),
        _ => None,
    }
}

// ===========================================================================
// Linux: /proc scanning + xdotool focus
// ===========================================================================

#[cfg(target_os = "linux")]
fn find_cli_processes() -> Vec<ProcInfo> {
    use std::path::Path;

    let mut out = Vec::new();
    let proc_dir = match std::fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => return out,
    };

    for entry in proc_dir.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let pid: u32 = match name_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let base = PathBuf::from("/proc").join(&*name_str);

        let cmdline = match std::fs::read(&base.join("cmdline")) {
            Ok(bytes) if !bytes.is_empty() => bytes,
            _ => continue,
        };

        let first_end = cmdline.iter().position(|&b| b == 0).unwrap_or(cmdline.len());
        let exe_path = String::from_utf8_lossy(&cmdline[..first_end]);
        let exe_name = Path::new(exe_path.as_ref())
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if exe_name != "claude" {
            continue;
        }

        let full = String::from_utf8_lossy(&cmdline);
        if full.contains("mcp-server") || full.contains("worker-service") {
            continue;
        }

        let stat = match std::fs::read_to_string(base.join("stat")) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let close = match stat.rfind(')') {
            Some(i) => i,
            None => continue,
        };
        let fields: Vec<&str> = stat[close + 2..].split_whitespace().collect();
        if fields.len() < 13 {
            continue;
        }

        let ppid: u32 = fields[1].parse().unwrap_or(0);
        let utime: u64 = fields[11].parse().unwrap_or(0);
        let stime: u64 = fields[12].parse().unwrap_or(0);

        let cwd = match std::fs::read_link(base.join("cwd")) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let tty_path = std::fs::read_link(base.join("fd/0"))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        if !tty_path.starts_with("/dev/pts/") && !tty_path.starts_with("/dev/tty") {
            continue;
        }

        let tty_label = tty_path
            .strip_prefix("/dev/")
            .unwrap_or(&tty_path)
            .to_string();

        out.push(ProcInfo {
            pid,
            ppid,
            cwd,
            tty_label,
            utime,
            stime,
            instant_cpu: None,
        });
    }

    out
}

#[cfg(target_os = "linux")]
impl Scanner {
    fn terminal_info(&mut self, p: &ProcInfo) -> Option<TerminalInfo> {
        use std::path::Path;

        let mut cur = p.ppid;
        let mut term_label = String::new();
        let mut term_pid: u32 = 0;

        for _ in 0..6 {
            if cur <= 1 {
                break;
            }
            let cmdline = std::fs::read_to_string(format!("/proc/{}/cmdline", cur)).ok()?;
            let exe = cmdline.split('\0').next().unwrap_or("");
            let exe_name = Path::new(exe)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            if let Some(label) = known_terminal(exe_name) {
                term_label = label.to_string();
                term_pid = cur;
                break;
            }

            // Go up
            let stat = std::fs::read_to_string(format!("/proc/{}/stat", cur)).ok()?;
            let close = stat.rfind(')')?;
            let fields: Vec<&str> = stat[close + 2..].split_whitespace().collect();
            cur = fields.get(1)?.parse().ok()?;
        }

        if term_pid == 0 {
            return Some(TerminalInfo {
                kind: "x11_generic".to_string(),
                focus_id: String::new(),
                outer_id: String::new(),
                label: p.tty_label.clone(),
                window_title: None,
            });
        }

        let focus_id = self
            .window_cache
            .entry(term_pid)
            .or_insert_with(|| xdotool_search_pid(term_pid))
            .clone()
            .unwrap_or_default();

        let window_title = if !focus_id.is_empty() {
            xdotool_get_name(&focus_id)
        } else {
            None
        };

        Some(TerminalInfo {
            kind: "x11_generic".to_string(),
            focus_id,
            outer_id: String::new(),
            label: term_label,
            window_title,
        })
    }
}

#[cfg(target_os = "linux")]
fn xdotool_get_name(wid_hex: &str) -> Option<String> {
    let output = std::process::Command::new("xdotool")
        .args(["getwindowname", wid_hex])
        .output()
        .ok()?;
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() { None } else { Some(name) }
}

#[cfg(target_os = "linux")]
fn xdotool_search_pid(pid: u32) -> Option<String> {
    let output = std::process::Command::new("xdotool")
        .args(["search", "--pid", &pid.to_string()])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let decimal = stdout.lines().next()?.trim();
    if decimal.is_empty() {
        return None;
    }
    let wid: u64 = decimal.parse().ok()?;
    Some(format!("0x{:x}", wid))
}

// ===========================================================================
// macOS: ps + lsof scanning, AppleScript focus
// ===========================================================================

#[cfg(target_os = "macos")]
fn find_cli_processes() -> Vec<ProcInfo> {
    use std::path::Path;

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

        if exe_name != "claude" {
            continue;
        }

        let full_cmd = parts[4..].join(" ");
        if full_cmd.contains("mcp-server") || full_cmd.contains("worker-service") {
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

        // Get CWD via lsof
        let cwd = lsof_cwd(pid).unwrap_or_else(|| PathBuf::from("/"));

        if tty_label.is_empty() {
            continue; // skip non-terminal processes
        }

        out.push(ProcInfo {
            pid,
            ppid,
            cwd,
            tty_label,
            utime: 0,
            stime: 0,
            instant_cpu: Some(cpu),
        });
    }

    out
}

#[cfg(target_os = "macos")]
fn lsof_cwd(pid: u32) -> Option<PathBuf> {
    let output = std::process::Command::new("lsof")
        .args(["-p", &pid.to_string(), "-d", "cwd", "-Fn"])
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

#[cfg(target_os = "macos")]
impl Scanner {
    fn terminal_info(&mut self, p: &ProcInfo) -> Option<TerminalInfo> {
        // On macOS, try to identify the terminal app from the parent chain
        let mut cur = p.ppid;
        let mut term_app = String::new();

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
            let exe_name = std::path::Path::new(exe)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            if let Some(label) = known_terminal(exe_name) {
                term_app = label.to_string();
                break;
            }

            cur = parts[0].trim().parse().unwrap_or(0);
        }

        if term_app.is_empty() {
            term_app = p.tty_label.clone();
        }

        Some(TerminalInfo {
            kind: "macos_app".to_string(),
            focus_id: term_app.clone(),
            outer_id: String::new(),
            label: term_app,
            window_title: None,
        })
    }
}

// ===========================================================================
// Windows: wmic/tasklist scanning
// ===========================================================================

#[cfg(target_os = "windows")]
fn find_cli_processes() -> Vec<ProcInfo> {
    let output = match std::process::Command::new("wmic")
        .args([
            "process",
            "where",
            "name='claude.exe'",
            "get",
            "ProcessId,ParentProcessId,CommandLine",
            "/FORMAT:CSV",
        ])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut out = Vec::new();

    for line in stdout.lines().skip(1) {
        let cols: Vec<&str> = line.split(',').collect();
        // CSV format: Node,CommandLine,ParentProcessId,ProcessId
        if cols.len() < 4 {
            continue;
        }

        let cmd_line = cols[1];
        if cmd_line.contains("mcp-server") || cmd_line.contains("worker-service") {
            continue;
        }

        let pid: u32 = match cols[3].trim().parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let ppid: u32 = cols[2].trim().parse().unwrap_or(0);

        // CWD is hard to get on Windows without elevated privileges
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("C:\\"));

        out.push(ProcInfo {
            pid,
            ppid,
            cwd,
            tty_label: String::new(),
            utime: 0,
            stime: 0,
            instant_cpu: None,
        });
    }

    out
}

#[cfg(target_os = "windows")]
impl Scanner {
    fn terminal_info(&mut self, _p: &ProcInfo) -> Option<TerminalInfo> {
        // Windows terminal focus requires win32 APIs (future enhancement)
        None
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_terminal_matches() {
        assert_eq!(known_terminal("kitty"), Some("Kitty"));
        assert_eq!(known_terminal("warp"), Some("Warp"));
        assert_eq!(known_terminal("iTerm2"), Some("iTerm2"));
        assert_eq!(known_terminal("bash"), None);
    }

    #[test]
    fn scanner_returns_vec() {
        let mut s = Scanner::new();
        let _agents = s.scan();
    }
}
