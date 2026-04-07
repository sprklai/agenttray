/// Focus a WezTerm pane via its CLI.
/// `focus_id` is the WEZTERM_PANE value (e.g. "3").

pub fn focus(focus_id: &str, _outer_id: &str) -> Result<(), String> {
    if focus_id.is_empty() {
        return Ok(());
    }

    use super::os_helpers::spawn_silent;

    spawn_silent("wezterm", &["cli", "activate-pane", "--pane-id", focus_id])
}
