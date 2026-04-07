use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use crate::watcher::AgentStatus;

pub mod strategies;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use self::linux as platform;
#[cfg(target_os = "macos")]
use self::macos as platform;
#[cfg(target_os = "windows")]
use self::windows as platform;

struct CpuSnapshot {
    total_ticks: u64,
    when: Instant,
    /// Last time this process had significant CPU activity.
    last_active: Option<Instant>,
}

/// Per-terminal-PID cache for window IDs (shared across platforms).
type WindowCache = HashMap<u32, Option<String>>;

/// Scans for live CLI agent processes across platforms.
pub struct Scanner {
    prev: HashMap<u32, CpuSnapshot>,
    window_cache: WindowCache,
    /// Per-PID cache for session IDs read from process environment.
    /// Session IDs are stable for a process's lifetime; caching avoids
    /// re-reading on every scan (important on Windows where env reading
    /// spawns a PowerShell process).
    session_id_cache: HashMap<u32, Option<String>>,
    /// Per-PID cache for multiplexer/CLI-terminal focus info detected from
    /// process environment variables (KITTY_WINDOW_ID, WEZTERM_PANE, etc.).
    /// Cached because env vars are stable for a process's lifetime and
    /// reading them can be expensive (macOS spawns a `ps` process).
    multiplexer_cache: HashMap<u32, Option<(String, String, String)>>,
    strategies: Vec<Box<dyn strategies::CliStrategy>>,
}

pub struct ProcInfo {
    pub pid: u32,
    pub ppid: u32,
    pub cwd: PathBuf,
    pub tty_label: String,
    pub utime: u64,
    pub stime: u64,
    /// Pre-computed CPU% (macOS `ps` gives this directly).
    pub instant_cpu: Option<f64>,
    /// Terminal window title (populated by platform terminal_info).
    pub window_title: Option<String>,
    /// Last time this process had significant CPU activity (carried from snapshot).
    pub last_active: Option<Instant>,
}

impl ProcInfo {
    /// Format the working directory as a home-relative path (e.g. `~/project`)
    /// when possible, falling back to the absolute path.
    pub fn cwd_display(&self) -> String {
        if let Some(home) = std::env::var_os("HOME") {
            if let Ok(rel) = self.cwd.strip_prefix(&home) {
                return format!("~/{}", rel.display());
            }
        }
        self.cwd.display().to_string()
    }
}

impl Scanner {
    pub fn new() -> Self {
        Self {
            prev: HashMap::new(),
            window_cache: HashMap::new(),
            session_id_cache: HashMap::new(),
            multiplexer_cache: HashMap::new(),
            strategies: strategies::all_strategies(),
        }
    }

    pub fn scan(&mut self) -> Vec<AgentStatus> {
        let (procs, ppid_map) = platform::find_cli_processes(&self.strategies);

        // Filter out subprocess agents: if ANY ancestor of a matched process
        // is also a matched CLI process, it's a subagent — not a user-level
        // session. We walk the full ancestor chain via ppid_map to catch
        // subagents spawned through intermediate processes (e.g.,
        // claude → bash → claude subagent).
        let matched_pids: std::collections::HashSet<u32> =
            procs.iter().map(|(p, _)| p.pid).collect();
        let procs: Vec<_> = procs
            .into_iter()
            .filter(|(p, _)| !has_matched_ancestor(p.ppid, &matched_pids, &ppid_map))
            .collect();

        let mut agents: Vec<AgentStatus> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        // Track which tty_labels we've already emitted to avoid duplicate IDs
        let mut seen_ttys: HashMap<String, usize> = HashMap::new();

        for (mut p, strategy) in procs {
            seen.insert(p.pid);

            let cpu_pct = self.cpu_pct(&p);
            let is_active = cpu_pct > 2.0;

            // Carry forward last_active from previous snapshot.
            // New processes start with last_active = now so they default
            // to "needs-input" rather than "idle" on first scan.
            let is_new = !self.prev.contains_key(&p.pid);
            let prev_last_active = self.prev.get(&p.pid).and_then(|s| s.last_active);
            let last_active = if is_active || is_new {
                Some(Instant::now())
            } else {
                prev_last_active
            };
            p.last_active = last_active;

            let project = p
                .cwd
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            // Get terminal info and populate window_title on ProcInfo
            let mut terminal = platform::terminal_info(&mut self.window_cache, &p);
            if let Some(ref t) = terminal {
                if p.window_title.is_none() {
                    p.window_title = t.window_title.clone();
                }
            }

            // Override kind/focus_id when the agent is running inside a terminal or
            // multiplexer with a CLI focus API (Kitty, WezTerm, tmux, Zellij, screen).
            // Env vars are stable for a process's lifetime, so results are cached.
            if let Some(ref mut t) = terminal {
                let mux = self.multiplexer_cache
                    .entry(p.pid)
                    .or_insert_with(|| detect_multiplexer_focus(p.pid));
                if let Some((ref kind, ref focus_id, ref outer_id)) = *mux {
                    t.kind = kind.clone();
                    t.focus_id = focus_id.clone();
                    t.outer_id = outer_id.clone();
                }
            }

            // Count direct child processes, excluding known background services
            // (MCP servers, worker services) that are always present.
            let child_count = count_children(p.pid, strategy.excluded_substrings());

            // Delegate state detection to the strategy
            let detected = strategy.detect_state(&p, cpu_pct, child_count);

            // Build display name from terminal label + tty (not window title, which
            // may contain transient status text like "Claude Code - Waiting for approval").
            let suffix = if let Some(ref t) = terminal {
                if !t.label.is_empty() && !p.tty_label.is_empty() {
                    format!("{} · {}", t.label, p.tty_label)
                } else if !t.label.is_empty() {
                    t.label.clone()
                } else {
                    p.tty_label.clone()
                }
            } else {
                p.tty_label.clone()
            };

            let name = if suffix.is_empty() {
                project.to_string()
            } else {
                format!("{} · {}", project, &suffix)
            };

            let can_focus = terminal
                .as_ref()
                .map(|t| !t.focus_id.is_empty())
                .unwrap_or(false);

            let id = format!("scan:{}", p.tty_label);

            // Read session ID from the process environment when the strategy
            // exposes an env var name (e.g. CLAUDE_SESSION_ID for Claude Code).
            // Cached per-PID: session IDs are stable for a process's lifetime,
            // and env reads can be expensive on Windows (spawns PowerShell).
            let session_id = strategy.session_env_var().and_then(|key| {
                self.session_id_cache
                    .entry(p.pid)
                    .or_insert_with(|| platform::read_proc_env(p.pid, key))
                    .clone()
            });

            let agent = AgentStatus {
                id,
                name,
                status: detected.status,
                message: detected.message,
                terminal,
                can_focus,
                cpu: Some(cpu_pct),
                source: Some("scan".into()),
                cli: Some(strategy.cli_name().to_string()),
                session_id,
                hook_event: None,
                hook_matcher: None,
                mtime: None,
            };

            // Dedup by tty: keep the higher-priority (lower numeric) status
            if let Some(&prev_idx) = seen_ttys.get(&p.tty_label) {
                let prev_prio = crate::watcher::status_priority_num(&agents[prev_idx].status);
                let cur_prio = crate::watcher::status_priority_num(&agent.status);
                if cur_prio < prev_prio {
                    agents[prev_idx] = agent;
                }
            } else {
                seen_ttys.insert(p.tty_label.clone(), agents.len());
                agents.push(agent);
            }

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
        self.session_id_cache.retain(|pid, _| seen.contains(pid));
        self.multiplexer_cache.retain(|pid, _| seen.contains(pid));
        self.window_cache.retain(|pid, _| seen.contains(pid));
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
        let clk_tck = clk_tck() as f64;
        (delta / clk_tck / dt) * 100.0
    }
}

/// Count direct child processes of a given PID, excluding children whose
/// command line contains any of the `excluded` substrings (e.g. "mcp-server",
/// "worker-service"). This prevents persistent background services from
/// inflating the child count and causing false "working" status.
#[cfg(target_os = "linux")]
fn count_children(pid: u32, excluded: &[&str]) -> u32 {
    let task_dir = format!("/proc/{}/task/{}/children", pid, pid);
    let children_str = match std::fs::read_to_string(&task_dir) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    if excluded.is_empty() {
        return children_str.split_whitespace().count() as u32;
    }

    children_str
        .split_whitespace()
        .filter(|child_pid_str| {
            let Ok(child_pid) = child_pid_str.parse::<u32>() else {
                return true;
            };
            let cmdline_path = format!("/proc/{}/cmdline", child_pid);
            let Ok(cmdline_bytes) = std::fs::read(&cmdline_path) else {
                return true;
            };
            let cmdline = String::from_utf8_lossy(&cmdline_bytes);
            !excluded.iter().any(|exc| cmdline.contains(exc))
        })
        .count() as u32
}

#[cfg(target_os = "macos")]
fn count_children(pid: u32, excluded: &[&str]) -> u32 {
    let output = match std::process::Command::new("pgrep")
        .args(["-P", &pid.to_string()])
        .output()
    {
        Ok(o) => o,
        Err(_) => return 0,
    };

    let child_pids: Vec<&str> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>();

    if excluded.is_empty() || child_pids.is_empty() {
        return child_pids.len() as u32;
    }

    // Batch all child PIDs into a single `ps` call instead of one per child.
    let pid_list = child_pids.join(",");
    let ps_out = match std::process::Command::new("ps")
        .args(["-o", "pid=,command=", "-p", &pid_list])
        .output()
    {
        Ok(o) => o,
        Err(_) => return child_pids.len() as u32,
    };
    let ps_text = String::from_utf8_lossy(&ps_out.stdout);

    // ps output: one line per matched PID ("  PID command...")
    // Count lines whose command does NOT match any excluded substring.
    ps_text
        .lines()
        .filter(|line| {
            let line = line.trim();
            if line.is_empty() {
                return false;
            }
            // Skip the PID field; everything after the first whitespace is the command.
            let cmd = line.splitn(2, char::is_whitespace).nth(1).unwrap_or(line);
            !excluded.iter().any(|exc| cmd.contains(exc))
        })
        .count() as u32
}

#[cfg(target_os = "windows")]
fn count_children(pid: u32, _excluded: &[&str]) -> u32 {
    // wmic is removed in Windows 11 24H2+; use Get-CimInstance instead.
    let script = format!(
        "(Get-CimInstance Win32_Process -Filter 'ParentProcessId={pid}').Count"
    );
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u32>().ok())
        .unwrap_or(0)
}

/// Walk up the process ancestor chain via ppid_map. Returns true if any
/// ancestor PID is in the `matched` set (i.e., the process is a descendant
/// of another matched CLI process — a subagent, not a user session).
fn has_matched_ancestor(
    ppid: u32,
    matched: &std::collections::HashSet<u32>,
    ppid_map: &HashMap<u32, u32>,
) -> bool {
    let mut cur = ppid;
    for _ in 0..15 {
        if cur <= 1 {
            return false;
        }
        if matched.contains(&cur) {
            return true;
        }
        cur = match ppid_map.get(&cur) {
            Some(&p) => p,
            None => return false,
        };
    }
    false
}

/// Returns the kernel clock ticks per second (CLK_TCK).
/// Reads from /proc on Linux; defaults to 100 on other platforms.
fn clk_tck() -> u64 {
    use std::sync::OnceLock;
    static CLK_TCK: OnceLock<u64> = OnceLock::new();
    *CLK_TCK.get_or_init(|| {
        #[cfg(target_os = "linux")]
        {
            // getconf CLK_TCK is the portable way without libc dependency
            std::process::Command::new("getconf")
                .arg("CLK_TCK")
                .output()
                .ok()
                .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
                .unwrap_or(100)
        }
        #[cfg(not(target_os = "linux"))]
        { 100 }
    })
}

// ===========================================================================
// Known terminal emulators (cross-platform lookup table).
// To add a new terminal, add one line here — no other changes needed.
// ===========================================================================

fn known_terminal(exe_name: &str) -> Option<&'static str> {
    match exe_name {
        // Cross-platform
        "warp" | "warp-terminal" | "Warp.exe" => Some("Warp"),
        "kitty" => Some("Kitty"),
        "alacritty" => Some("Alacritty"),
        "ghostty" | "Ghostty" => Some("Ghostty"),
        "WezTerm" | "wezterm" | "wezterm-gui" => Some("WezTerm"),
        "hyper" | "Hyper" => Some("Hyper"),
        "tabby" | "Tabby" => Some("Tabby"),
        // IDE terminals (process names)
        "Code" | "code" | "Electron" => Some("Visual Studio Code"),
        // Linux
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
        // macOS
        "Terminal" => Some("Terminal"),
        "iTerm2" => Some("iTerm2"),
        // Windows
        "WindowsTerminal.exe" | "WindowsTerminal" => Some("Windows Terminal"),
        "ConEmuC.exe" | "ConEmuC64.exe" | "ConEmu.exe" => Some("ConEmu"),
        "cmd.exe" | "cmd" => Some("CMD"),
        "powershell.exe" | "powershell" | "pwsh.exe" | "pwsh" => Some("PowerShell"),
        _ => None,
    }
}

// ===========================================================================
// Multiplexer / CLI-terminal focus detection via process environment variables.
// Checks env vars in priority order; first match wins.
// ===========================================================================

/// Returns `Some((kind, focus_id, outer_id))` if the agent process (identified by
/// `pid`) is running inside a terminal or multiplexer that has a CLI-based focus
/// API. Returns `None` if only the generic platform focuser should be used.
fn detect_multiplexer_focus(pid: u32) -> Option<(String, String, String)> {
    // Kitty: KITTY_WINDOW_ID is the integer window ID for `kitty @ focus-window`.
    if let Some(id) = platform::read_proc_env(pid, "KITTY_WINDOW_ID") {
        if !id.is_empty() {
            return Some(("kitty".into(), id, String::new()));
        }
    }
    // WezTerm: WEZTERM_PANE is the pane ID for `wezterm cli activate-pane`.
    if let Some(id) = platform::read_proc_env(pid, "WEZTERM_PANE") {
        if !id.is_empty() {
            return Some(("wezterm".into(), id, String::new()));
        }
    }
    // tmux: TMUX_PANE is the pane reference (e.g. "%3"); tmux select-pane accepts this.
    if let Some(id) = platform::read_proc_env(pid, "TMUX_PANE") {
        if !id.is_empty() {
            return Some(("tmux".into(), id, String::new()));
        }
    }
    // Zellij: ZELLIJ_SESSION_NAME is the session name for `zellij attach`.
    if let Some(id) = platform::read_proc_env(pid, "ZELLIJ_SESSION_NAME") {
        if !id.is_empty() {
            return Some(("zellij".into(), id, String::new()));
        }
    }
    // GNU Screen: STY is the session identifier; WINDOW is the window number.
    if let Some(sty) = platform::read_proc_env(pid, "STY") {
        if !sty.is_empty() {
            let win = platform::read_proc_env(pid, "WINDOW").unwrap_or_default();
            return Some(("screen".into(), sty, win));
        }
    }
    None
}

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

    #[test]
    fn ancestor_direct_parent_matched() {
        // claude(100) → claude(200): ppid=100 is matched → filtered
        let matched: std::collections::HashSet<u32> = [100, 200].into();
        let ppid_map: HashMap<u32, u32> = [(200, 100)].into();
        assert!(has_matched_ancestor(100, &matched, &ppid_map));
    }

    #[test]
    fn ancestor_through_intermediate() {
        // claude(100) → bash(150) → claude(200): walk 200→150→100, 100 matched
        let matched: std::collections::HashSet<u32> = [100, 200].into();
        let ppid_map: HashMap<u32, u32> = [(200, 150), (150, 100), (100, 1)].into();
        assert!(has_matched_ancestor(150, &matched, &ppid_map));
    }

    #[test]
    fn ancestor_no_match() {
        // terminal(50) → bash(150) → claude(200): no matched ancestor
        let matched: std::collections::HashSet<u32> = [200].into();
        let ppid_map: HashMap<u32, u32> = [(200, 150), (150, 50), (50, 1)].into();
        assert!(!has_matched_ancestor(150, &matched, &ppid_map));
    }

    #[test]
    fn ancestor_stops_at_init() {
        // Process with ppid=1 (init) — no ancestor match
        let matched: std::collections::HashSet<u32> = [100].into();
        let ppid_map: HashMap<u32, u32> = [(100, 1)].into();
        assert!(!has_matched_ancestor(1, &matched, &ppid_map));
    }

    #[test]
    fn all_strategies_contains_claude() {
        let strats = strategies::all_strategies();
        assert!(!strats.is_empty());
        assert!(strats[0].process_names().contains(&"claude"));
    }
}
