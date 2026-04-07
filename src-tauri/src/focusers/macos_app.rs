/// Activates a macOS terminal window using AppleScript.
/// `focus_id` is the application name (e.g. "iTerm2", "Terminal").
/// `outer_id` is the TTY name (e.g. "ttys000") for tab-specific focus.

#[cfg(target_os = "macos")]
fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

pub fn focus(focus_id: &str, outer_id: &str) -> Result<(), String> {
    if focus_id.is_empty() {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        use super::os_helpers::spawn_silent;

        // If we have a TTY, try tab-specific focus via terminal's AppleScript API
        if !outer_id.is_empty() {
            let safe_outer = escape_applescript(outer_id);
            let script = match focus_id {
                "iTerm2" => Some(format!(
                    r#"tell application "iTerm2"
                        activate
                        repeat with w in windows
                            repeat with t in tabs of w
                                repeat with s in sessions of t
                                    if tty of s contains "{}" then
                                        select s
                                        return
                                    end if
                                end repeat
                            end repeat
                        end repeat
                    end tell"#,
                    safe_outer
                )),
                "Terminal" => Some(format!(
                    r#"tell application "Terminal"
                        activate
                        repeat with w in windows
                            repeat with t in tabs of w
                                if tty of t contains "{}" then
                                    set selected tab of w to t
                                    set index of w to 1
                                    return
                                end if
                            end repeat
                        end repeat
                    end tell"#,
                    safe_outer
                )),
                _ => None,
            };

            if let Some(script) = script {
                return spawn_silent("osascript", &["-e", &script]);
            }
        }

        // Fallback: just activate the app
        let app = escape_applescript(focus_id);
        let script = format!("tell application \"{}\" to activate", app);
        spawn_silent("osascript", &["-e", &script])?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (focus_id, outer_id);
        log::debug!("macos_app focus is only supported on macOS");
    }

    Ok(())
}
