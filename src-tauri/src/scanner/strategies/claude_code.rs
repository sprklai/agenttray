use super::{CliStrategy, DetectedState};
use crate::scanner::ProcInfo;

pub struct ClaudeCodeStrategy;

/// Seconds after last high-CPU burst before we fall back to "idle".
const NEEDS_INPUT_WINDOW_SECS: u64 = 120;

impl CliStrategy for ClaudeCodeStrategy {
    fn process_names(&self) -> &[&str] {
        &["claude"]
    }

    fn excluded_substrings(&self) -> &[&str] {
        &["mcp-server", "worker-service"]
    }

    fn detect_state(&self, info: &ProcInfo, cpu_pct: f64, child_count: u32) -> DetectedState {
        // Signal 1: Window title patterns (highest confidence).
        // Claude Code sets terminal title; some terminals expose it.
        if let Some(ref title) = info.window_title {
            if let Some(state) = detect_from_title(title) {
                return state;
            }
        }

        // Signal 2: Child processes — if claude spawned subprocesses
        // (bash, git, npm, cargo, etc.), it's executing a tool.
        if child_count > 0 && cpu_pct > 0.5 {
            return DetectedState {
                status: "working".to_string(),
                message: format!("Running tool ({} subprocess{})", child_count, if child_count == 1 { "" } else { "es" }),
                confidence: 0.75,
            };
        }

        // Signal 3: CPU heuristic (fallback).
        let is_active = cpu_pct > 2.0;

        if is_active {
            DetectedState {
                status: "working".to_string(),
                message: format!("Active ({:.0}% CPU)", cpu_pct),
                confidence: 0.5,
            }
        } else if let Some(t) = info.last_active {
            if t.elapsed().as_secs() < NEEDS_INPUT_WINDOW_SECS {
                DetectedState {
                    status: "needs-input".to_string(),
                    message: "Waiting for input".to_string(),
                    confidence: 0.4,
                }
            } else {
                DetectedState {
                    status: "idle".to_string(),
                    message: info.cwd.display().to_string(),
                    confidence: 0.4,
                }
            }
        } else {
            DetectedState {
                status: "idle".to_string(),
                message: info.cwd.display().to_string(),
                confidence: 0.3,
            }
        }
    }

    fn tool_label(&self) -> &str {
        "Claude Code"
    }

    fn cli_name(&self) -> &str {
        "claude-code"
    }
}

/// Try to infer state from the terminal window title.
/// Claude Code updates the terminal title during operation.
fn detect_from_title(title: &str) -> Option<DetectedState> {
    let lower = title.to_lowercase();

    // Claude Code shows "Claude Code" or task description in title.
    // When waiting for input, the prompt indicator appears.
    // When working, tool names or "thinking" may appear.

    // Patterns observed: title often contains the prompt "❯" when idle/waiting
    if lower.contains("waiting") || lower.contains("approve") || lower.contains("allow") {
        return Some(DetectedState {
            status: "needs-input".to_string(),
            message: truncate(title, 120),
            confidence: 0.9,
        });
    }

    if lower.contains("running") || lower.contains("editing") || lower.contains("thinking") {
        return Some(DetectedState {
            status: "working".to_string(),
            message: truncate(title, 120),
            confidence: 0.9,
        });
    }

    None
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        match s.char_indices().nth(max) {
            Some((idx, _)) => s[..idx].to_string(),
            None => s.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Instant;

    fn make_info(title: Option<&str>, last_active: Option<Instant>) -> ProcInfo {
        ProcInfo {
            pid: 1000,
            ppid: 999,
            cwd: PathBuf::from("/home/user/project"),
            tty_label: "pts/1".to_string(),
            utime: 0,
            stime: 0,
            instant_cpu: None,
            window_title: title.map(|s| s.to_string()),
            last_active,
        }
    }

    #[test]
    fn high_cpu_means_working() {
        let strategy = ClaudeCodeStrategy;
        let info = make_info(None, None);
        let state = strategy.detect_state(&info, 15.0, 0);
        assert_eq!(state.status, "working");
    }

    #[test]
    fn low_cpu_recently_active_means_needs_input() {
        let strategy = ClaudeCodeStrategy;
        let info = make_info(None, Some(Instant::now()));
        let state = strategy.detect_state(&info, 0.5, 0);
        assert_eq!(state.status, "needs-input");
    }

    #[test]
    fn low_cpu_long_idle_means_idle() {
        let strategy = ClaudeCodeStrategy;
        let info = make_info(None, None);
        let state = strategy.detect_state(&info, 0.5, 0);
        assert_eq!(state.status, "idle");
    }

    #[test]
    fn child_processes_means_working() {
        let strategy = ClaudeCodeStrategy;
        let info = make_info(None, None);
        let state = strategy.detect_state(&info, 1.0, 2);
        assert_eq!(state.status, "working");
        assert!(state.message.contains("subprocess"));
    }

    #[test]
    fn title_with_waiting_means_needs_input() {
        let strategy = ClaudeCodeStrategy;
        let info = make_info(Some("Claude Code - Waiting for approval"), None);
        let state = strategy.detect_state(&info, 10.0, 0);
        assert_eq!(state.status, "needs-input");
        assert!(state.confidence > 0.8);
    }

    #[test]
    fn title_with_running_means_working() {
        let strategy = ClaudeCodeStrategy;
        let info = make_info(Some("Claude Code - Running tests"), None);
        let state = strategy.detect_state(&info, 0.1, 0);
        assert_eq!(state.status, "working");
        assert!(state.confidence > 0.8);
    }

    #[test]
    fn process_names_match() {
        let s = ClaudeCodeStrategy;
        assert!(s.process_names().contains(&"claude"));
    }

    #[test]
    fn excluded_substrings_filter() {
        let s = ClaudeCodeStrategy;
        assert!(s.excluded_substrings().contains(&"mcp-server"));
        assert!(s.excluded_substrings().contains(&"worker-service"));
    }
}
