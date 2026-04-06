/// Focus a GNU Screen session/window.
/// `focus_id` is the STY value (e.g. "12345.pts-0.hostname").
/// `outer_id` is the window number (e.g. "0").

pub fn focus(focus_id: &str, outer_id: &str) -> Result<(), String> {
    if focus_id.is_empty() {
        return Ok(());
    }

    use super::os_helpers::spawn_silent;

    // Reattach to the session and select the window
    let win = if outer_id.is_empty() { "0" } else { outer_id };
    spawn_silent("screen", &["-x", focus_id, "-p", win])
}
