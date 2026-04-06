/// Focus a Kitty terminal window via its remote-control protocol.
/// `focus_id` is the KITTY_WINDOW_ID (integer string).

pub fn focus(focus_id: &str, _outer_id: &str) -> Result<(), String> {
    if focus_id.is_empty() {
        return Ok(());
    }

    use super::os_helpers::spawn_silent;

    // kitty @ focus-window --match id:<window_id>
    spawn_silent(
        "kitty",
        &["@", "focus-window", "--match", &format!("id:{}", focus_id)],
    )
}
