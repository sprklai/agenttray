use super::{CliStrategy, DetectedState, TitlePattern};
use crate::scanner::ProcInfo;

/// Strategy for Google Gemini CLI agent detection.
pub struct GeminiStrategy;

/// Gemini CLI sets dynamic status icons in the terminal title via OSC:
/// ◇ = Ready, ✋ = Action Required, ✦ = Working
const TITLE_PATTERNS: &[TitlePattern] = &[
    // Unicode status icons (primary signal — documented Gemini feature)
    TitlePattern { pattern: "\u{270B}", status: "needs-input", confidence: 0.95 }, // ✋
    TitlePattern { pattern: "\u{2726}", status: "working", confidence: 0.9 },      // ✦
    TitlePattern { pattern: "\u{25C7}", status: "idle", confidence: 0.85 },        // ◇
    // Text fallbacks
    TitlePattern { pattern: "Action Required", status: "needs-input", confidence: 0.9 },
];

impl CliStrategy for GeminiStrategy {
    fn process_names(&self) -> &[&str] {
        &["gemini"]
    }

    fn excluded_substrings(&self) -> &[&str] {
        &[]
    }

    fn script_names(&self) -> &[&str] {
        &["gemini"]
    }

    fn detect_state(&self, info: &ProcInfo, cpu_pct: f64, child_count: u32) -> DetectedState {
        // Signal 1: Window title status icons (highest confidence).
        // Gemini CLI sets ✋/✦/◇ in the terminal title.
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
        "Gemini"
    }

    fn cli_name(&self) -> &str {
        "gemini"
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
        let s = GeminiStrategy;
        assert!(s.process_names().contains(&"gemini"));
    }

    #[test]
    fn tool_label() {
        assert_eq!(GeminiStrategy.tool_label(), "Gemini");
    }

    #[test]
    fn script_names_contains_gemini() {
        let s = GeminiStrategy;
        assert!(s.script_names().contains(&"gemini"));
    }

    #[test]
    fn title_hand_icon_means_needs_input() {
        let s = GeminiStrategy;
        let info = make_info(Some("✋ Gemini"));
        let state = s.detect_state(&info, 0.0, 0);
        assert_eq!(state.status, "needs-input");
        assert!(state.confidence > 0.9);
    }

    #[test]
    fn title_star_icon_means_working() {
        let s = GeminiStrategy;
        let info = make_info(Some("✦ Gemini"));
        let state = s.detect_state(&info, 0.0, 0);
        assert_eq!(state.status, "working");
    }

    #[test]
    fn title_diamond_icon_means_idle() {
        let s = GeminiStrategy;
        let info = make_info(Some("◇ Gemini"));
        let state = s.detect_state(&info, 0.0, 0);
        assert_eq!(state.status, "idle");
    }

    #[test]
    fn title_action_required_means_needs_input() {
        let s = GeminiStrategy;
        let info = make_info(Some("Action Required - review changes"));
        let state = s.detect_state(&info, 0.0, 0);
        assert_eq!(state.status, "needs-input");
    }

    #[test]
    fn no_title_falls_back_to_cpu() {
        let s = GeminiStrategy;
        let info = make_info(None);
        let state = s.detect_state(&info, 10.0, 0);
        assert_eq!(state.status, "working");
    }
}
