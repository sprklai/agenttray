use std::time::{Duration, SystemTime};

use crate::watcher::AgentStatus;

/// How old a hook status file must be before we consider it stale.
const STALE_WORKING_TTL: Duration = Duration::from_secs(30);

/// How long a permission_prompt needs-input can stay unacknowledged before
/// we assume the permission was granted but the follow-up hook didn't fire.
const STALE_PERMISSION_TTL: Duration = Duration::from_secs(60);

/// CPU threshold below which a "working" hook status is suspect.
const LOW_CPU_THRESHOLD: f64 = 5.0;

/// CPU threshold above which we override an "idle"/"offline" hook status.
const HIGH_CPU_THRESHOLD: f64 = 10.0;

/// Cross-validate hook-based agent statuses against live process scanner data.
///
/// This acts as a second heuristic layer:
/// - Detects stale "working" hooks when the process is clearly idle
/// - Detects missed activity when hooks say "idle" but the process is active
/// - Never overrides "needs-input" or "error" (these are explicit signals)
///
/// Called after dedup, before sorting and emission.
pub fn cross_validate(agents: &mut Vec<AgentStatus>, scanned: &[AgentStatus]) {
    let now = SystemTime::now();

    for agent in agents.iter_mut() {
        // Only cross-validate hook-sourced agents
        if agent.source.as_deref() != Some("hook") {
            continue;
        }

        // "error" is always a definitive state — never override it.
        if agent.status == "error" {
            continue;
        }

        let Some(scan) = find_scanner_match(agent, scanned) else {
            continue;
        };

        let scan_cpu = scan.cpu.unwrap_or(0.0);

        match agent.status.as_str() {
            // needs-input: only permission_prompt and elicitation_dialog are
            // genuine user-actionable states. All other matchers (unknown
            // Notification types like quota_warning, context_compact, etc.)
            // can fire after Stop and spuriously overwrite an idle status —
            // clear them once stale. Permission prompts are only cleared if
            // the process has resumed (CPU active), meaning the permission
            // was silently granted (common on macOS).
            "needs-input" => {
                let matcher = agent.hook_matcher.as_deref().unwrap_or("");
                let is_genuine = matcher == "permission_prompt"
                    || matcher == "elicitation_dialog";
                let is_stale = agent.mtime
                    .and_then(|mt| now.duration_since(mt).ok())
                    .map(|age| age > STALE_PERMISSION_TTL)
                    .unwrap_or(false);
                if is_genuine {
                    if is_stale && scan_cpu > LOW_CPU_THRESHOLD {
                        agent.status = "working".to_string();
                        agent.message = "Running tool...".to_string();
                    }
                } else if is_stale {
                    agent.status = "idle".to_string();
                    agent.message = "Waiting for input".to_string();
                }
            }
            "working" | "starting" => {
                // Stale working: hook says active but process CPU is near-zero
                // and the status file hasn't been updated recently
                let is_stale = agent.mtime
                    .and_then(|mt| now.duration_since(mt).ok())
                    .map(|age| age > STALE_WORKING_TTL)
                    .unwrap_or(false);

                if is_stale && scan_cpu < LOW_CPU_THRESHOLD {
                    agent.status = "idle".to_string();
                    agent.message = format!("{} (stale hook, low CPU)", agent.message);
                }
            }
            "idle" | "offline" => {
                // Missed activity: hook says idle but process is clearly active
                if scan_cpu > HIGH_CPU_THRESHOLD {
                    agent.status = "working".to_string();
                    agent.message = format!("Active ({:.0}% CPU)", scan_cpu);
                }
            }
            _ => {}
        }
    }
}

/// Find the scanner-detected agent that corresponds to a hook-based agent.
///
/// Correlation strategy (in priority order):
/// 1. Same CLI name + same terminal focus_id
/// 2. Same CLI name when there's exactly one scanner result for that CLI
fn find_scanner_match<'a>(agent: &AgentStatus, scanned: &'a [AgentStatus]) -> Option<&'a AgentStatus> {
    let cli = agent.cli.as_deref()?;
    let agent_focus_id = agent.terminal.as_ref()
        .map(|t| t.focus_id.as_str())
        .unwrap_or("");

    // Try matching by CLI + focus_id first
    if !agent_focus_id.is_empty() && agent_focus_id != "0" {
        if let Some(m) = scanned.iter().find(|s| {
            s.cli.as_deref() == Some(cli)
                && s.terminal.as_ref()
                    .map(|t| t.focus_id == agent_focus_id)
                    .unwrap_or(false)
        }) {
            return Some(m);
        }
    }

    // Fallback: if exactly one scanner result for this CLI, use it
    let cli_matches: Vec<&AgentStatus> = scanned.iter()
        .filter(|s| s.cli.as_deref() == Some(cli))
        .collect();
    if cli_matches.len() == 1 {
        return Some(cli_matches[0]);
    }

    None
}

/// Read I/O counters from /proc/{pid}/io (Linux only).
/// Returns (read_bytes, write_bytes) for delta-based activity detection.
#[cfg(target_os = "linux")]
#[allow(dead_code)]
pub fn proc_io(pid: u32) -> Option<(u64, u64)> {
    let content = std::fs::read_to_string(format!("/proc/{}/io", pid)).ok()?;
    let mut read = 0u64;
    let mut write = 0u64;
    for line in content.lines() {
        if let Some(val) = line.strip_prefix("read_bytes: ") {
            read = val.trim().parse().unwrap_or(0);
        }
        if let Some(val) = line.strip_prefix("write_bytes: ") {
            write = val.trim().parse().unwrap_or(0);
        }
    }
    Some((read, write))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watcher::TerminalInfo;

    fn hook_agent(status: &str, cli: &str, focus_id: &str, mtime: Option<SystemTime>) -> AgentStatus {
        AgentStatus {
            id: format!("file:test-{}", cli),
            name: "test".into(),
            status: status.into(),
            message: "test message".into(),
            terminal: Some(TerminalInfo {
                kind: "x11_generic".into(),
                focus_id: focus_id.into(),
                outer_id: "".into(),
                label: "Terminal".into(),
                window_title: None,
            }),
            can_focus: !focus_id.is_empty(),
            cpu: None,
            source: Some("hook".into()),
            cli: Some(cli.into()),
            session_id: None,
            hook_event: None,
            hook_matcher: None,
            mtime,
        }
    }

    fn scan_agent(cli: &str, focus_id: &str, cpu: f64) -> AgentStatus {
        AgentStatus {
            id: format!("scan:pts/1"),
            name: "scan-test".into(),
            status: "idle".into(),
            message: "".into(),
            terminal: Some(TerminalInfo {
                kind: "x11_generic".into(),
                focus_id: focus_id.into(),
                outer_id: "".into(),
                label: "Terminal".into(),
                window_title: None,
            }),
            can_focus: !focus_id.is_empty(),
            cpu: Some(cpu),
            source: Some("scan".into()),
            cli: Some(cli.into()),
            session_id: None,
            hook_event: None,
            hook_matcher: None,
            mtime: None,
        }
    }

    #[test]
    fn stale_working_hook_downgraded_to_idle() {
        let old_mtime = SystemTime::now() - Duration::from_secs(60);
        let mut agents = vec![hook_agent("working", "claude-code", "0x1234", Some(old_mtime))];
        let scanned = vec![scan_agent("claude-code", "0x1234", 0.0)];

        cross_validate(&mut agents, &scanned);
        assert_eq!(agents[0].status, "idle");
    }

    #[test]
    fn fresh_working_hook_not_downgraded() {
        let fresh_mtime = SystemTime::now();
        let mut agents = vec![hook_agent("working", "claude-code", "0x1234", Some(fresh_mtime))];
        let scanned = vec![scan_agent("claude-code", "0x1234", 0.0)];

        cross_validate(&mut agents, &scanned);
        assert_eq!(agents[0].status, "working");
    }

    #[test]
    fn idle_hook_upgraded_when_cpu_high() {
        let mut agents = vec![hook_agent("idle", "gemini", "0x5678", None)];
        let scanned = vec![scan_agent("gemini", "0x5678", 15.0)];

        cross_validate(&mut agents, &scanned);
        assert_eq!(agents[0].status, "working");
    }

    #[test]
    fn non_genuine_stale_needs_input_cleared_to_idle() {
        // hook_matcher=None means unknown/non-genuine source — stale entries
        // should be downgraded to idle regardless of CPU.
        let old_mtime = SystemTime::now() - Duration::from_secs(120);
        let mut agents = vec![hook_agent("needs-input", "claude-code", "0x1234", Some(old_mtime))];
        let scanned = vec![scan_agent("claude-code", "0x1234", 0.0)];

        cross_validate(&mut agents, &scanned);
        assert_eq!(agents[0].status, "idle");
    }

    #[test]
    fn error_never_overridden() {
        let mut agents = vec![hook_agent("error", "codex", "0x1234", None)];
        let scanned = vec![scan_agent("codex", "0x1234", 20.0)];

        cross_validate(&mut agents, &scanned);
        assert_eq!(agents[0].status, "error");
    }

    #[test]
    fn no_scanner_match_leaves_hook_unchanged() {
        let old_mtime = SystemTime::now() - Duration::from_secs(60);
        let mut agents = vec![hook_agent("working", "claude-code", "0x1234", Some(old_mtime))];
        let scanned = vec![scan_agent("gemini", "0x9999", 0.0)]; // different CLI

        cross_validate(&mut agents, &scanned);
        assert_eq!(agents[0].status, "working"); // unchanged
    }

    #[test]
    fn scan_source_agents_not_cross_validated() {
        let mut agents = vec![AgentStatus {
            id: "scan:pts/0".into(),
            name: "scan-only".into(),
            status: "working".into(),
            message: "".into(),
            terminal: None,
            can_focus: false,
            cpu: Some(0.0),
            source: Some("scan".into()),
            cli: Some("claude-code".into()),
            session_id: None,
            hook_event: None,
            hook_matcher: None,
            mtime: None,
        }];
        let scanned = vec![];

        cross_validate(&mut agents, &scanned);
        assert_eq!(agents[0].status, "working"); // scan-sourced, not touched
    }

    #[test]
    fn fallback_single_cli_match() {
        // Hook agent has no focus_id, but there's exactly one scanner result for the same CLI
        let old_mtime = SystemTime::now() - Duration::from_secs(60);
        let mut agents = vec![hook_agent("working", "codex", "", Some(old_mtime))];
        let scanned = vec![scan_agent("codex", "0xABCD", 0.1)];

        cross_validate(&mut agents, &scanned);
        assert_eq!(agents[0].status, "idle"); // stale + low CPU → downgraded
    }

    fn hook_agent_with_matcher(
        status: &str, cli: &str, focus_id: &str,
        matcher: Option<&str>, mtime: Option<SystemTime>,
    ) -> AgentStatus {
        let mut a = hook_agent(status, cli, focus_id, mtime);
        a.hook_matcher = matcher.map(|s| s.into());
        a
    }

    #[test]
    fn stale_permission_prompt_cleared_when_process_active() {
        // Simulates macOS case: permission granted, PreToolUse hook didn't fire.
        let old_mtime = SystemTime::now() - Duration::from_secs(90);
        let mut agents = vec![hook_agent_with_matcher(
            "needs-input", "claude-code", "0x1234",
            Some("permission_prompt"), Some(old_mtime),
        )];
        let scanned = vec![scan_agent("claude-code", "0x1234", 6.0)]; // CPU active (above LOW_CPU_THRESHOLD)

        cross_validate(&mut agents, &scanned);
        assert_eq!(agents[0].status, "working");
    }

    #[test]
    fn fresh_permission_prompt_not_cleared() {
        // Permission prompt is recent — user hasn't responded yet.
        let fresh_mtime = SystemTime::now();
        let mut agents = vec![hook_agent_with_matcher(
            "needs-input", "claude-code", "0x1234",
            Some("permission_prompt"), Some(fresh_mtime),
        )];
        let scanned = vec![scan_agent("claude-code", "0x1234", 3.0)];

        cross_validate(&mut agents, &scanned);
        assert_eq!(agents[0].status, "needs-input"); // too fresh to clear
    }

    #[test]
    fn non_genuine_needs_input_cleared_when_stale() {
        // Spurious Notification types (quota_warning, unknown, etc.) that
        // wrote needs-input should be downgraded to idle once stale.
        let old_mtime = SystemTime::now() - Duration::from_secs(90);
        let mut agents = vec![hook_agent_with_matcher(
            "needs-input", "claude-code", "0x1234",
            Some("quota_warning"), Some(old_mtime),
        )];
        let scanned = vec![scan_agent("claude-code", "0x1234", 0.0)];

        cross_validate(&mut agents, &scanned);
        assert_eq!(agents[0].status, "idle");
    }

    #[test]
    fn non_genuine_needs_input_kept_when_fresh() {
        // Even a non-genuine needs-input should be kept if still fresh.
        let fresh_mtime = SystemTime::now();
        let mut agents = vec![hook_agent_with_matcher(
            "needs-input", "claude-code", "0x1234",
            Some("quota_warning"), Some(fresh_mtime),
        )];
        let scanned = vec![scan_agent("claude-code", "0x1234", 0.0)];

        cross_validate(&mut agents, &scanned);
        assert_eq!(agents[0].status, "needs-input");
    }
}
