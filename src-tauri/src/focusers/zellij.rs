/// Focus a Zellij session.
/// `focus_id` is the session name (from ZELLIJ_SESSION_NAME).

pub fn focus(focus_id: &str, _outer_id: &str) -> Result<(), String> {
    if focus_id.is_empty() {
        return Ok(());
    }

    use super::os_helpers::spawn_silent;

    // Attach to the named session (brings it to focus)
    spawn_silent("zellij", &["attach", focus_id])
}
