mod claude_code;
mod codex;
mod gemini;

pub use claude_code::ClaudeCodeStrategy;
pub use codex::CodexStrategy;
pub use gemini::GeminiStrategy;

use super::ProcInfo;

/// Runtimes that invoke scripts as argv[1]. When argv[0] matches one of
/// these, the scanner checks subsequent args against `script_names()`.
/// To support a new runtime, add one entry here — no other changes needed.
pub const SCRIPT_RUNTIMES: &[&str] = &[
    "node", "nodejs", "bun", "deno", "npx", "pnpx", "tsx", "ts-node",
    "python", "python3",
];

/// A window title substring → agent status mapping.
/// Strategies return a list of these; checked in order, first match wins.
pub struct TitlePattern {
    pub pattern: &'static str,
    pub status: &'static str,
    pub confidence: f32,
}

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

    /// Script names to match in argv[1..] when argv[0] is a known runtime
    /// (node, bun, deno, etc.). For native binaries this returns empty.
    /// To detect a new Node.js CLI, implement this method.
    fn script_names(&self) -> &[&str] {
        &[]
    }

    /// Window title patterns that indicate a specific agent state.
    /// Each CLI sets terminal titles differently via OSC escape sequences.
    /// Patterns are checked in order; first match wins.
    fn title_patterns(&self) -> &[TitlePattern] {
        &[]
    }
}

/// Match a window title against a strategy's title patterns.
/// Returns the first matching DetectedState, or None.
pub fn detect_from_title(title: &str, patterns: &[TitlePattern]) -> Option<DetectedState> {
    let lower = title.to_lowercase();
    for p in patterns {
        if lower.contains(&p.pattern.to_lowercase()) {
            let msg = if title.len() <= 120 {
                title.to_string()
            } else {
                title.char_indices().nth(120).map_or_else(
                    || title.to_string(),
                    |(idx, _)| title[..idx].to_string(),
                )
            };
            return Some(DetectedState {
                status: p.status.to_string(),
                message: msg,
                confidence: p.confidence,
            });
        }
    }
    None
}

/// Returns all registered CLI strategies.
pub fn all_strategies() -> Vec<Box<dyn CliStrategy>> {
    vec![
        Box::new(ClaudeCodeStrategy),
        Box::new(CodexStrategy),
        Box::new(GeminiStrategy),
    ]
}
