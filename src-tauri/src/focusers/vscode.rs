/// Focus VS Code by re-activating its window.
/// Uses `code --reuse-window` which brings the existing window to front.

pub fn focus(_focus_id: &str, _outer_id: &str) -> Result<(), String> {
    use super::os_helpers::spawn_silent;

    // --reuse-window brings the current VS Code window to the foreground
    spawn_silent("code", &["--reuse-window"])
}
