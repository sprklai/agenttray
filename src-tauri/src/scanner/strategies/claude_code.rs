use super::{CliStrategy, DetectedState, TitlePattern};
use crate::scanner::ProcInfo;

pub struct ClaudeCodeStrategy;

/// Claude Code title patterns. Claude Code sets terminal title via OSC but
/// does not currently use dynamic status indicators (no ✋/✦ icons).
/// These patterns match keywords that may appear in the title text.
const TITLE_PATTERNS: &[TitlePattern] = &[
    // needs-input signals
    TitlePattern { pattern: "approv",     status: "needs-input", confidence: 0.9 },
    TitlePattern { pattern: "allow",      status: "needs-input", confidence: 0.9 },
    TitlePattern { pattern: "permission", status: "needs-input", confidence: 0.85 },
    TitlePattern { pattern: "yes/no",     status: "needs-input", confidence: 0.85 },
    TitlePattern { pattern: "bypass",     status: "needs-input", confidence: 0.8 },
    // working signals
    TitlePattern { pattern: "running",    status: "working", confidence: 0.9 },
    TitlePattern { pattern: "editing",    status: "working", confidence: 0.9 },
    TitlePattern { pattern: "thinking",   status: "working", confidence: 0.9 },
    TitlePattern { pattern: "interrupt",  status: "working", confidence: 0.85 },
];

impl CliStrategy for ClaudeCodeStrategy {
    fn process_names(&self) -> &[&str] {
        &["claude"]
    }

    fn excluded_substrings(&self) -> &[&str] {
        &["mcp-server", "worker-service"]
    }

    fn detect_state(&self, info: &ProcInfo, cpu_pct: f64, child_count: u32) -> DetectedState {
        // Signal 1: Window title patterns (highest confidence).
        if let Some(ref title) = info.window_title {
            if let Some(state) = super::detect_from_title(title, self.title_patterns()) {
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
        if cpu_pct > 2.0 {
            DetectedState {
                status: "working".to_string(),
                message: format!("Active ({:.0}% CPU)", cpu_pct),
                confidence: 0.5,
            }
        } else {
            DetectedState {
                status: "idle".to_string(),
                message: info.cwd_display(),
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

    fn title_patterns(&self) -> &[TitlePattern] {
        TITLE_PATTERNS
    }

    fn session_env_var(&self) -> Option<&str> {
        Some("CLAUDE_SESSION_ID")
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
    fn low_cpu_recently_active_means_idle() {
        let strategy = ClaudeCodeStrategy;
        let info = make_info(None, Some(Instant::now()));
        let state = strategy.detect_state(&info, 0.5, 0);
        assert_eq!(state.status, "idle");
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
    fn title_with_waiting_for_input_means_idle() {
        // "Waiting for input" = idle prompt after task completion, not needs-input
        let strategy = ClaudeCodeStrategy;
        let info = make_info(Some("Claude Code - Waiting for input"), None);
        let state = strategy.detect_state(&info, 0.1, 0);
        assert_eq!(state.status, "idle");
    }

    #[test]
    fn title_with_waiting_for_approval_means_needs_input() {
        // "approve" pattern still fires for permission requests
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
