use super::{CliStrategy, DetectedState};
use crate::scanner::ProcInfo;

/// Strategy for Google Gemini CLI agent detection.
pub struct GeminiStrategy;

impl CliStrategy for GeminiStrategy {
    fn process_names(&self) -> &[&str] {
        &["gemini"]
    }

    fn excluded_substrings(&self) -> &[&str] {
        &[]
    }

    fn detect_state(&self, info: &ProcInfo, cpu_pct: f64, child_count: u32) -> DetectedState {
        if child_count > 0 && cpu_pct > 0.5 {
            return DetectedState {
                status: "working".to_string(),
                message: format!("Running tool ({} subprocess{})", child_count, if child_count == 1 { "" } else { "es" }),
                confidence: 0.7,
            };
        }

        if cpu_pct > 2.0 {
            DetectedState {
                status: "working".to_string(),
                message: format!("Active ({:.0}% CPU)", cpu_pct),
                confidence: 0.5,
            }
        } else if let Some(t) = info.last_active {
            if t.elapsed().as_secs() < 120 {
                DetectedState {
                    status: "needs-input".to_string(),
                    message: "Waiting for input".to_string(),
                    confidence: 0.4,
                }
            } else {
                DetectedState {
                    status: "idle".to_string(),
                    message: info.cwd.display().to_string(),
                    confidence: 0.3,
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
        "Gemini"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_names() {
        let s = GeminiStrategy;
        assert!(s.process_names().contains(&"gemini"));
    }

    #[test]
    fn tool_label() {
        assert_eq!(GeminiStrategy.tool_label(), "Gemini");
    }
}
