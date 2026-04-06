use super::{CliStrategy, DetectedState, TitlePattern};
use crate::scanner::ProcInfo;

/// Strategy for OpenAI Codex CLI agent detection.
/// Codex is a Rust binary distributed via npm wrapper (node bin/codex.js → native binary)
/// or as a standalone binary from GitHub Releases.
pub struct CodexStrategy;

/// Codex CLI sets dynamic status labels in terminal title via /title command:
/// Starting..., Ready, Thinking..., Working..., Waiting..., Undoing...
const TITLE_PATTERNS: &[TitlePattern] = &[
    TitlePattern { pattern: "Waiting", status: "needs-input", confidence: 0.95 },
    TitlePattern { pattern: "Thinking", status: "working", confidence: 0.9 },
    TitlePattern { pattern: "Working", status: "working", confidence: 0.9 },
    TitlePattern { pattern: "Undoing", status: "working", confidence: 0.85 },
    TitlePattern { pattern: "Starting", status: "starting", confidence: 0.85 },
    TitlePattern { pattern: "Ready", status: "idle", confidence: 0.85 },
];

impl CliStrategy for CodexStrategy {
    fn process_names(&self) -> &[&str] {
        // The npm wrapper spawns a native Rust binary. Process name varies:
        // - "codex" (standalone or symlinked)
        // - Platform-specific names from GitHub Releases
        &["codex", "codex-linux-x64", "codex-linux-arm64",
          "codex-darwin-x64", "codex-darwin-arm64", "codex-win-x64"]
    }

    fn excluded_substrings(&self) -> &[&str] {
        &[]
    }

    fn script_names(&self) -> &[&str] {
        &["codex", "codex.js"]
    }

    fn detect_state(&self, info: &ProcInfo, cpu_pct: f64, child_count: u32) -> DetectedState {
        // Signal 1: Window title status labels (highest confidence).
        // Codex CLI sets "Thinking...", "Working...", "Waiting..." etc. in title.
        if let Some(ref title) = info.window_title {
            if let Some(state) = super::detect_from_title(title, self.title_patterns()) {
                return state;
            }
        }

        // Signal 2: Child processes indicate tool execution.
        if child_count > 0 && cpu_pct > 0.5 {
            return DetectedState {
                status: "working".to_string(),
                message: format!("Running tool ({} subprocess{})", child_count, if child_count == 1 { "" } else { "es" }),
                confidence: 0.7,
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
                message: info.cwd.display().to_string(),
                confidence: 0.3,
            }
        }
    }

    fn tool_label(&self) -> &str {
        "Codex"
    }

    fn cli_name(&self) -> &str {
        "codex"
    }

    fn title_patterns(&self) -> &[TitlePattern] {
        TITLE_PATTERNS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_info(title: Option<&str>) -> ProcInfo {
        ProcInfo {
            pid: 1000, ppid: 999,
            cwd: PathBuf::from("/home/user/project"),
            tty_label: "pts/1".to_string(),
            utime: 0, stime: 0,
            instant_cpu: None,
            window_title: title.map(|s| s.to_string()),
            last_active: None,
        }
    }

    #[test]
    fn process_names() {
        let s = CodexStrategy;
        assert!(s.process_names().contains(&"codex"));
    }

    #[test]
    fn process_names_include_platform_binaries() {
        let s = CodexStrategy;
        assert!(s.process_names().contains(&"codex-linux-x64"));
        assert!(s.process_names().contains(&"codex-darwin-arm64"));
    }

    #[test]
    fn tool_label() {
        assert_eq!(CodexStrategy.tool_label(), "Codex");
    }

    #[test]
    fn script_names_contains_codex() {
        let s = CodexStrategy;
        assert!(s.script_names().contains(&"codex"));
        assert!(s.script_names().contains(&"codex.js"));
    }

    #[test]
    fn title_waiting_means_needs_input() {
        let s = CodexStrategy;
        let info = make_info(Some("Waiting... · myproject"));
        let state = s.detect_state(&info, 0.0, 0);
        assert_eq!(state.status, "needs-input");
        assert!(state.confidence > 0.9);
    }

    #[test]
    fn title_thinking_means_working() {
        let s = CodexStrategy;
        let info = make_info(Some("Thinking... · myproject"));
        let state = s.detect_state(&info, 0.0, 0);
        assert_eq!(state.status, "working");
    }

    #[test]
    fn title_working_means_working() {
        let s = CodexStrategy;
        let info = make_info(Some("Working... · myproject"));
        let state = s.detect_state(&info, 0.0, 0);
        assert_eq!(state.status, "working");
    }

    #[test]
    fn title_ready_means_idle() {
        let s = CodexStrategy;
        let info = make_info(Some("Ready · myproject"));
        let state = s.detect_state(&info, 0.0, 0);
        assert_eq!(state.status, "idle");
    }

    #[test]
    fn title_starting_means_starting() {
        let s = CodexStrategy;
        let info = make_info(Some("Starting... · myproject"));
        let state = s.detect_state(&info, 0.0, 0);
        assert_eq!(state.status, "starting");
    }

    #[test]
    fn no_title_falls_back_to_cpu() {
        let s = CodexStrategy;
        let info = make_info(None);
        let state = s.detect_state(&info, 10.0, 0);
        assert_eq!(state.status, "working");
    }
}
