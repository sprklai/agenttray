use std::time::{Duration, SystemTime};

use crate::watcher::AgentStatus;

/// How old a hook status file must be before we consider it stale.
const STALE_WORKING_TTL: Duration = Duration::from_secs(30);

/// How long a permission_prompt needs-input can stay unacknowledged before
/// we assume the session has moved on (either the permission was granted or
/// the task completed without a follow-up hook clearing the state).
const STALE_PERMISSION_TTL: Duration = Duration::from_secs(30);

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

        // Scanner-detected waiting states are higher confidence than a stale
        // hook file saying "working" or "idle", so upgrade immediately.
        if scan.status == "needs-input" && agent.status != "needs-input" {
            agent.status = "needs-input".to_string();
            agent.message = if scan.message.is_empty() {
                "Waiting for input".to_string()
            } else {
                scan.message.clone()
            };
            continue;
        }

        match agent.status.as_str() {
            // needs-input: preserve scanner-confirmed waits and known genuine
            // hook waits (Codex Stop, Gemini Notification, Claude permission /
            // elicitation prompts). Only Claude's permission-style waits auto-
            // resolve when they go stale; non-genuine hook waits are cleared.
            "needs-input" => {
                if scan.status == "needs-input" {
                    continue;
                }

                let is_claude_permission_wait = matches!(
                    agent.cli.as_deref(),
                    Some("claude-code")
                ) && matches!(
                    agent.hook_matcher.as_deref(),
                    Some("permission_prompt" | "elicitation_dialog")
                );
                let is_genuine = is_genuine_needs_input(agent);
                let is_stale = agent.mtime
                    .and_then(|mt| now.duration_since(mt).ok())
                    .map(|age| age > STALE_PERMISSION_TTL)
                    .unwrap_or(false);
                if is_claude_permission_wait && is_stale {
                    if scan_cpu > LOW_CPU_THRESHOLD {
                        // CPU active → permission was silently granted, process resumed
                        agent.status = "working".to_string();
                        agent.message = "Running tool...".to_string();
                    } else {
                        // CPU idle → task is done; stale permission prompt is moot
                        agent.status = "idle".to_string();
                        agent.message = "Waiting for input".to_string();
                    }
                } else if !is_genuine && is_stale {
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

fn is_genuine_needs_input(agent: &AgentStatus) -> bool {
    match agent.cli.as_deref() {
        Some("claude-code") => matches!(
            agent.hook_matcher.as_deref(),
            Some("permission_prompt" | "elicitation_dialog")
        ),
        Some("codex") => agent.hook_event.as_deref() == Some("Stop"),
        Some("gemini") => agent.hook_event.as_deref() == Some("Notification"),
        _ => false,
    }
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

    fn scan_agent_with_status(cli: &str, focus_id: &str, status: &str, cpu: f64) -> AgentStatus {
        AgentStatus {
            id: format!("scan:pts/1"),
            name: "scan-test".into(),
            status: status.into(),
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

    fn scan_agent(cli: &str, focus_id: &str, cpu: f64) -> AgentStatus {
        scan_agent_with_status(cli, focus_id, "idle", cpu)
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

    fn hook_agent_with_event(
        status: &str, cli: &str, focus_id: &str,
        hook_event: Option<&str>, matcher: Option<&str>, mtime: Option<SystemTime>,
    ) -> AgentStatus {
        let mut a = hook_agent_with_matcher(status, cli, focus_id, matcher, mtime);
        a.hook_event = hook_event.map(|s| s.into());
        a
    }

    #[test]
    fn stale_permission_prompt_cleared_when_process_active() {
        // Simulates macOS case: permission granted, PreToolUse hook didn't fire.
        let old_mtime = SystemTime::now() - Duration::from_secs(45);
        let mut agents = vec![hook_agent_with_matcher(
            "needs-input", "claude-code", "0x1234",
            Some("permission_prompt"), Some(old_mtime),
        )];
        let scanned = vec![scan_agent("claude-code", "0x1234", 6.0)]; // CPU active (above LOW_CPU_THRESHOLD)

        cross_validate(&mut agents, &scanned);
        assert_eq!(agents[0].status, "working");
    }

    #[test]
    fn stale_permission_prompt_cleared_to_idle_when_cpu_low() {
        // Task completed, but permission_prompt notification raced past Stop.
        // After STALE_PERMISSION_TTL with low CPU, should resolve to idle.
        let old_mtime = SystemTime::now() - Duration::from_secs(45);
        let mut agents = vec![hook_agent_with_matcher(
            "needs-input", "claude-code", "0x1234",
            Some("permission_prompt"), Some(old_mtime),
        )];
        let scanned = vec![scan_agent("claude-code", "0x1234", 0.5)]; // CPU idle

        cross_validate(&mut agents, &scanned);
        assert_eq!(agents[0].status, "idle");
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

    #[test]
    fn working_hook_upgraded_when_scanner_detects_waiting() {
        let mut agents = vec![hook_agent("working", "codex", "0x1234", None)];
        let scanned = vec![scan_agent_with_status("codex", "0x1234", "needs-input", 0.0)];

        cross_validate(&mut agents, &scanned);
        assert_eq!(agents[0].status, "needs-input");
    }

    #[test]
    fn stale_codex_stop_wait_not_cleared() {
        let old_mtime = SystemTime::now() - Duration::from_secs(90);
        let mut agents = vec![hook_agent_with_event(
            "needs-input", "codex", "0x1234",
            Some("Stop"), None, Some(old_mtime),
        )];
        let scanned = vec![scan_agent("codex", "0x1234", 0.0)];

        cross_validate(&mut agents, &scanned);
        assert_eq!(agents[0].status, "needs-input");
    }
}
