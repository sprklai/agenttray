/// Activates a macOS terminal window using AppleScript.
/// `focus_id` is the application name (e.g. "iTerm2", "Terminal").

pub fn focus(focus_id: &str, _outer_id: &str) -> Result<(), String> {
    if focus_id.is_empty() {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        use super::os_helpers::spawn_silent;

        // Activate the application by name via AppleScript
        let script = format!(
            "tell application \"{}\" to activate",
            focus_id.replace('\\', "\\\\").replace('"', "\\\"")
        );
        spawn_silent("osascript", &["-e", &script])?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = focus_id;
        log::debug!("macos_app focus is only supported on macOS");
    }

    Ok(())
}
