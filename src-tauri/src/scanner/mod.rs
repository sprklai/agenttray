use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use crate::watcher::AgentStatus;

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

/// Scans for live Claude CLI processes across platforms.
pub struct Scanner {
    prev: HashMap<u32, CpuSnapshot>,
    window_cache: WindowCache,
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
            window_cache: HashMap::new(),
        }
    }

    pub fn scan(&mut self) -> Vec<AgentStatus> {
        let procs = platform::find_cli_processes();
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

            // Get terminal info first so we can use window title in the name
            let terminal = platform::terminal_info(&mut self.window_cache, p);

            // Build a display suffix for the agent name.
            // Window titles are unique per window; when unavailable,
            // combine terminal label + tty for disambiguation.
            let suffix = if let Some(ref t) = terminal {
                if let Some(ref wt) = t.window_title {
                    wt.clone()
                } else if !t.label.is_empty() && !p.tty_label.is_empty() {
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

            let message = match status {
                "working" => format!("Active ({:.0}% CPU)", cpu_pct),
                "needs-input" => "Waiting for input".to_string(),
                _ => p.cwd.display().to_string(),
            };

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
        let clk_tck = clk_tck() as f64;
        (delta / clk_tck / dt) * 100.0
    }
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
