use crate::watcher::AgentStatus;

/// Events that can trigger a notification.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    NeedsInput { agent_name: String },
}

/// Swappable notification backend.
pub trait Notifier: Send + Sync {
    fn notify(&self, event: &AgentEvent);
}

/// Plays a platform system sound via shell commands.
pub struct SystemBeepNotifier;

impl Notifier for SystemBeepNotifier {
    fn notify(&self, event: &AgentEvent) {
        match event {
            AgentEvent::NeedsInput { agent_name } => {
                log::info!("Agent '{}' needs input — playing alert", agent_name);
                play_system_beep();
            }
        }
    }
}

fn play_system_beep() {
    std::thread::spawn(|| {
        if let Err(e) = platform_beep() {
            log::warn!("Failed to play notification sound: {}", e);
        }
    });
}

#[cfg(target_os = "linux")]
fn platform_beep() -> Result<(), String> {
    use std::process::Command;

    let sounds = [
        "/usr/share/sounds/freedesktop/stereo/bell.oga",
        "/usr/share/sounds/freedesktop/stereo/complete.oga",
    ];
    for path in &sounds {
        if std::path::Path::new(path).exists() {
            if Command::new("paplay").arg(path).output().is_ok() {
                return Ok(());
            }
        }
    }
    // Last resort
    Command::new("sh")
        .args(["-c", "printf '\\a'"])
        .output()
        .map(|_| ())
        .map_err(|e| format!("beep failed: {}", e))
}

#[cfg(target_os = "macos")]
fn platform_beep() -> Result<(), String> {
    use std::process::Command;
    Command::new("afplay")
        .arg("/System/Library/Sounds/Glass.aiff")
        .output()
        .map(|_| ())
        .map_err(|e| format!("afplay failed: {}", e))
}

#[cfg(target_os = "windows")]
fn platform_beep() -> Result<(), String> {
    use std::process::Command;
    Command::new("powershell")
        .args(["-c", "[System.Media.SystemSounds]::Exclamation.Play()"])
        .output()
        .map(|_| ())
        .map_err(|e| format!("powershell beep failed: {}", e))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn platform_beep() -> Result<(), String> {
    log::debug!("No platform beep implementation for this OS");
    Ok(())
}

/// Compare old and new agent lists; fire notifications for transitions to needs-input.
/// Uses `id` for identity matching so title/name changes don't retrigger alerts.
pub fn detect_and_notify(old: &[AgentStatus], new: &[AgentStatus], notifier: &dyn Notifier) {
    for agent in new {
        if agent.status != "needs-input" {
            continue;
        }
        let was_needs_input = old
            .iter()
            .find(|a| a.id == agent.id)
            .map(|a| a.status == "needs-input")
            .unwrap_or(false);

        if !was_needs_input {
            notifier.notify(&AgentEvent::NeedsInput {
                agent_name: agent.name.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingNotifier(AtomicUsize);
    impl Notifier for CountingNotifier {
        fn notify(&self, _event: &AgentEvent) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn agent(name: &str, status: &str) -> AgentStatus {
        AgentStatus {
            id: format!("test:{}", name),
            name: name.into(),
            status: status.into(),
            message: String::new(),
            terminal: None,
            can_focus: false,
            cpu: None,
        }
    }

    #[test]
    fn no_notification_when_already_needs_input() {
        let n = CountingNotifier(AtomicUsize::new(0));
        let old = vec![agent("a", "needs-input")];
        let new = vec![agent("a", "needs-input")];
        detect_and_notify(&old, &new, &n);
        assert_eq!(n.0.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn notification_on_transition_to_needs_input() {
        let n = CountingNotifier(AtomicUsize::new(0));
        let old = vec![agent("a", "working")];
        let new = vec![agent("a", "needs-input")];
        detect_and_notify(&old, &new, &n);
        assert_eq!(n.0.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn notification_for_new_agent_as_needs_input() {
        let n = CountingNotifier(AtomicUsize::new(0));
        detect_and_notify(&[], &[agent("a", "needs-input")], &n);
        assert_eq!(n.0.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn no_notification_for_non_needs_input() {
        let n = CountingNotifier(AtomicUsize::new(0));
        let old = vec![agent("a", "idle")];
        let new = vec![agent("a", "working")];
        detect_and_notify(&old, &new, &n);
        assert_eq!(n.0.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn multiple_agents_only_transitioned_ones_notify() {
        let n = CountingNotifier(AtomicUsize::new(0));
        let old = vec![
            agent("a", "working"),
            agent("b", "needs-input"),
            agent("c", "idle"),
        ];
        let new = vec![
            agent("a", "needs-input"),
            agent("b", "needs-input"),
            agent("c", "needs-input"),
        ];
        detect_and_notify(&old, &new, &n);
        assert_eq!(n.0.load(Ordering::Relaxed), 2);
    }
}
