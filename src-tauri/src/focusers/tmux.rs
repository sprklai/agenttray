/// Focus a tmux pane.
/// `focus_id` is the pane target: "session:window.pane" (e.g. "main:0.1").

pub fn focus(focus_id: &str, _outer_id: &str) -> Result<(), String> {
    if focus_id.is_empty() {
        return Ok(());
    }

    use super::os_helpers::spawn_silent;

    // Select the target pane — this also switches the tmux client to show it
    spawn_silent("tmux", &["select-pane", "-t", focus_id])?;
    spawn_silent("tmux", &["select-window", "-t", focus_id])
}
