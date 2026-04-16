use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

use crate::watcher::AgentStatus;

const NEEDS_INPUT_REMINDER_INTERVAL: Duration = Duration::from_secs(10);

/// Events that can trigger a notification.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    NeedsInput {
        agent_name: String,
        /// Hook-provided reason: "permission_prompt", "idle_prompt", "elicitation_dialog", etc.
        reason: Option<String>,
    },
    NeedsInputReminder {
        agent_name: String,
        reason: Option<String>,
    },
}

/// Swappable notification backend.
pub trait Notifier: Send + Sync {
    fn notify(&self, event: &AgentEvent, app: Option<&AppHandle>);
}

/// Plays a platform system sound via shell commands.
pub struct SystemBeepNotifier;

impl Notifier for SystemBeepNotifier {
    fn notify(&self, event: &AgentEvent, _app: Option<&AppHandle>) {
        match event {
            AgentEvent::NeedsInput { agent_name, reason } => {
                let reason_str = reason.as_deref().unwrap_or("unknown");
                log::info!("Agent '{}' needs input ({}) — playing alert", agent_name, reason_str);
                play_system_beep();
            }
            AgentEvent::NeedsInputReminder { agent_name, reason } => {
                let reason_str = reason.as_deref().unwrap_or("unknown");
                log::info!("Agent '{}' still needs input ({}) — playing reminder", agent_name, reason_str);
                play_system_beep();
            }
        }
    }
}

/// Sends native OS desktop notifications via tauri-plugin-notification.
pub struct DesktopNotifier;

impl Notifier for DesktopNotifier {
    fn notify(&self, event: &AgentEvent, app: Option<&AppHandle>) {
        let Some(app) = app else {
            log::warn!("DesktopNotifier: no AppHandle available, skipping notification");
            return;
        };
        match event {
            AgentEvent::NeedsInput { agent_name, reason } => {
                let body = match reason.as_deref() {
                    Some(r) => format!("{} needs your input ({})", agent_name, r),
                    None => format!("{} needs your input", agent_name),
                };
                let mut builder = app.notification()
                    .builder()
                    .title("AgentTray")
                    .body(&body);
                if let Some(icon_path) = notification_icon_path() {
                    builder = builder.icon(icon_path);
                }
                if let Err(e) = builder.show() {
                    log::warn!("Desktop notification failed: {}", e);
                }
            }
            AgentEvent::NeedsInputReminder { .. } => {}
        }
    }
}

/// Returns an absolute path to the notification icon, writing it to a cache
/// file on first call. On Linux, notify_rust needs an absolute path or
/// freedesktop icon name; in dev mode the app isn't installed, so auto_icon()
/// fails. We embed the 128x128 app icon at compile time and materialize it.
fn notification_icon_path() -> Option<String> {
    let cache_dir = dirs_next::cache_dir()?.join("agent-tray");
    let icon_path = cache_dir.join("notification-icon.png");
    if !icon_path.exists() {
        std::fs::create_dir_all(&cache_dir).ok()?;
        std::fs::write(&icon_path, include_bytes!("../icons/128x128.png")).ok()?;
    }
    Some(icon_path.to_string_lossy().into_owned())
}

/// Fires multiple notifiers in sequence (beep + desktop banner).
pub struct CompositeNotifier {
    backends: Vec<Box<dyn Notifier>>,
}

impl CompositeNotifier {
    pub fn new(backends: Vec<Box<dyn Notifier>>) -> Self {
        Self { backends }
    }
}

impl Notifier for CompositeNotifier {
    fn notify(&self, event: &AgentEvent, app: Option<&AppHandle>) {
        for backend in &self.backends {
            backend.notify(event, app);
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ReminderState {
    entered_at: Instant,
    last_reminder_at: Option<Instant>,
}

#[derive(Default)]
struct NeedsInputReminderTracker {
    active: HashMap<String, ReminderState>,
}

impl NeedsInputReminderTracker {
    fn collect_events(&mut self, old: &[AgentStatus], new: &[AgentStatus], now: Instant) -> Vec<AgentEvent> {
        let active_ids: HashSet<String> = new.iter()
            .filter(|agent| agent.status == "needs-input")
            .map(|agent| agent.id.clone())
            .collect();
        self.active.retain(|id, _| active_ids.contains(id));

        let mut events = Vec::new();

        for agent in new.iter().filter(|agent| agent.status == "needs-input") {
            let was_needs_input = old
                .iter()
                .find(|old_agent| old_agent.id == agent.id)
                .map(|old_agent| old_agent.status == "needs-input")
                .unwrap_or(false);

            let entry = self.active.entry(agent.id.clone()).or_insert(ReminderState {
                entered_at: now,
                last_reminder_at: None,
            });

            if !was_needs_input {
                *entry = ReminderState {
                    entered_at: now,
                    last_reminder_at: None,
                };
                events.push(AgentEvent::NeedsInput {
                    agent_name: agent.name.clone(),
                    reason: agent.hook_matcher.clone(),
                });
                continue;
            }

            let has_waited_long_enough = now.duration_since(entry.entered_at) >= NEEDS_INPUT_REMINDER_INTERVAL;
            let reminder_due = entry.last_reminder_at
                .map(|last| now.duration_since(last) >= NEEDS_INPUT_REMINDER_INTERVAL)
                .unwrap_or(true);

            if has_waited_long_enough && reminder_due {
                entry.last_reminder_at = Some(now);
                events.push(AgentEvent::NeedsInputReminder {
                    agent_name: agent.name.clone(),
                    reason: agent.hook_matcher.clone(),
                });
            }
        }

        events
    }
}

static REMINDER_TRACKER: LazyLock<Mutex<NeedsInputReminderTracker>> = LazyLock::new(|| {
    Mutex::new(NeedsInputReminderTracker::default())
});

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

    // Try freedesktop sound theme (uses user's configured sound theme)
    if Command::new("canberra-gtk-play")
        .args(["--id", "bell"])
        .output()
        .map_or(false, |o| o.status.success())
    {
        return Ok(());
    }

    // Try paplay with XDG sound lookup
    if Command::new("paplay")
        .args(["--property", "media.role=event"])
        .arg("/usr/share/sounds/freedesktop/stereo/bell.oga")
        .output()
        .map_or(false, |o| o.status.success())
    {
        return Ok(());
    }

    // Last resort: terminal bell
    Command::new("sh")
        .args(["-c", "printf '\\a'"])
        .output()
        .map(|_| ())
        .map_err(|e| format!("beep failed: {}", e))
}

#[cfg(target_os = "macos")]
fn platform_beep() -> Result<(), String> {
    use std::process::Command;
    // Use AppleScript beep — plays the user's configured alert sound
    // without hardcoding any file path
    Command::new("osascript")
        .args(["-e", "beep"])
        .output()
        .map(|_| ())
        .map_err(|e| format!("osascript beep failed: {}", e))
}

#[cfg(target_os = "windows")]
fn platform_beep() -> Result<(), String> {
    use std::process::Command;
    // Uses .NET system sounds — plays the user's configured exclamation sound
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

/// Compare old and new agent lists; fire immediate notifications for
/// transitions to needs-input, then periodic reminder beeps while the
/// waiting state persists. Uses `id` for identity matching so title/name
/// changes don't retrigger alerts.
pub fn detect_and_notify(old: &[AgentStatus], new: &[AgentStatus], notifier: &dyn Notifier, app: Option<&AppHandle>) {
    let mut tracker = REMINDER_TRACKER.lock().unwrap_or_else(|e| {
        log::warn!("REMINDER_TRACKER mutex poisoned, recovering");
        e.into_inner()
    });
    detect_and_notify_with_tracker(old, new, notifier, app, &mut tracker, Instant::now());
}

fn detect_and_notify_with_tracker(
    old: &[AgentStatus],
    new: &[AgentStatus],
    notifier: &dyn Notifier,
    app: Option<&AppHandle>,
    tracker: &mut NeedsInputReminderTracker,
    now: Instant,
) {
    for event in tracker.collect_events(old, new, now) {
        notifier.notify(&event, app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingNotifier(AtomicUsize);
    impl Notifier for CountingNotifier {
        fn notify(&self, _event: &AgentEvent, _app: Option<&AppHandle>) {
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
            source: None,
            cli: None,
            session_id: None,
            hook_event: None,
            hook_matcher: None,
            mtime: None,
        }
    }

    #[test]
    fn no_notification_when_already_needs_input() {
        let n = CountingNotifier(AtomicUsize::new(0));
        let mut tracker = NeedsInputReminderTracker::default();
        let old = vec![agent("a", "needs-input")];
        let new = vec![agent("a", "needs-input")];
        detect_and_notify_with_tracker(&old, &new, &n, None, &mut tracker, Instant::now());
        assert_eq!(n.0.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn notification_on_transition_to_needs_input() {
        let n = CountingNotifier(AtomicUsize::new(0));
        let mut tracker = NeedsInputReminderTracker::default();
        let old = vec![agent("a", "working")];
        let new = vec![agent("a", "needs-input")];
        detect_and_notify_with_tracker(&old, &new, &n, None, &mut tracker, Instant::now());
        assert_eq!(n.0.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn notification_for_new_agent_as_needs_input() {
        let n = CountingNotifier(AtomicUsize::new(0));
        let mut tracker = NeedsInputReminderTracker::default();
        detect_and_notify_with_tracker(&[], &[agent("a", "needs-input")], &n, None, &mut tracker, Instant::now());
        assert_eq!(n.0.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn no_notification_for_non_needs_input() {
        let n = CountingNotifier(AtomicUsize::new(0));
        let mut tracker = NeedsInputReminderTracker::default();
        let old = vec![agent("a", "idle")];
        let new = vec![agent("a", "working")];
        detect_and_notify_with_tracker(&old, &new, &n, None, &mut tracker, Instant::now());
        assert_eq!(n.0.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn multiple_agents_only_transitioned_ones_notify() {
        let n = CountingNotifier(AtomicUsize::new(0));
        let mut tracker = NeedsInputReminderTracker::default();
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
        detect_and_notify_with_tracker(&old, &new, &n, None, &mut tracker, Instant::now());
        assert_eq!(n.0.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn reminder_fires_after_ten_seconds() {
        let n = CountingNotifier(AtomicUsize::new(0));
        let mut tracker = NeedsInputReminderTracker::default();
        let start = Instant::now();
        let waiting = vec![agent("a", "needs-input")];

        detect_and_notify_with_tracker(&[], &waiting, &n, None, &mut tracker, start);
        detect_and_notify_with_tracker(&waiting, &waiting, &n, None, &mut tracker, start + Duration::from_secs(10));

        assert_eq!(n.0.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn reminder_repeats_every_ten_seconds() {
        let n = CountingNotifier(AtomicUsize::new(0));
        let mut tracker = NeedsInputReminderTracker::default();
        let start = Instant::now();
        let waiting = vec![agent("a", "needs-input")];

        detect_and_notify_with_tracker(&[], &waiting, &n, None, &mut tracker, start);
        detect_and_notify_with_tracker(&waiting, &waiting, &n, None, &mut tracker, start + Duration::from_secs(9));
        detect_and_notify_with_tracker(&waiting, &waiting, &n, None, &mut tracker, start + Duration::from_secs(10));
        detect_and_notify_with_tracker(&waiting, &waiting, &n, None, &mut tracker, start + Duration::from_secs(19));
        detect_and_notify_with_tracker(&waiting, &waiting, &n, None, &mut tracker, start + Duration::from_secs(20));

        assert_eq!(n.0.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn reminder_state_clears_when_agent_leaves_waiting() {
        let n = CountingNotifier(AtomicUsize::new(0));
        let mut tracker = NeedsInputReminderTracker::default();
        let start = Instant::now();
        let waiting = vec![agent("a", "needs-input")];
        let working = vec![agent("a", "working")];

        detect_and_notify_with_tracker(&[], &waiting, &n, None, &mut tracker, start);
        detect_and_notify_with_tracker(&waiting, &working, &n, None, &mut tracker, start + Duration::from_secs(5));
        detect_and_notify_with_tracker(&working, &waiting, &n, None, &mut tracker, start + Duration::from_secs(6));

        assert_eq!(n.0.load(Ordering::Relaxed), 2);
    }
}
