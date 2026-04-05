mod claude_code;
mod codex;
mod gemini;

pub use claude_code::ClaudeCodeStrategy;
pub use codex::CodexStrategy;
pub use gemini::GeminiStrategy;

use super::ProcInfo;

/// Result of a CLI strategy's state detection.
pub struct DetectedState {
    pub status: String,
    pub message: String,
    /// How confident we are (0.0–1.0). Higher-confidence results
    /// are preferred when multiple signals conflict.
    #[allow(dead_code)]
    pub confidence: f32,
}

/// Trait for CLI-specific agent detection strategies.
///
/// Each supported CLI tool (Claude Code, Codex, Gemini, etc.) implements
/// this trait. The scanner iterates all registered strategies to find
/// and classify running agent processes.
pub trait CliStrategy: Send + Sync {
    /// Executable names to match (e.g., `["claude"]`).
    fn process_names(&self) -> &[&str];

    /// Substrings in the full command line that disqualify a process
    /// (e.g., helper processes like `mcp-server`).
    fn excluded_substrings(&self) -> &[&str];

    /// Determine the agent state from process info.
    /// `cpu_pct` is pre-computed by the scanner's CPU delta tracker.
    /// `child_count` is the number of direct child processes.
    fn detect_state(&self, info: &ProcInfo, cpu_pct: f64, child_count: u32) -> DetectedState;

    /// Human-readable tool label shown in the UI (e.g., "Claude Code").
    #[allow(dead_code)]
    fn tool_label(&self) -> &str;

    /// Machine-readable CLI name for status files (e.g., "claude-code").
    fn cli_name(&self) -> &str;
}

/// Returns all registered CLI strategies.
pub fn all_strategies() -> Vec<Box<dyn CliStrategy>> {
    vec![
        Box::new(ClaudeCodeStrategy),
        Box::new(CodexStrategy),
        Box::new(GeminiStrategy),
    ]
}
