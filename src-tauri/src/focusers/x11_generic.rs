#[cfg(target_os = "linux")]
use super::os_helpers;

pub fn focus(focus_id: &str, _outer_id: &str) -> Result<(), String> {
    if focus_id.is_empty() {
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        // focus_id format: "WINDOWID:SHELLPID"
        let (window_id, shell_pid) = match focus_id.split_once(':') {
            Some((w, p)) => (w, p),
            None => (focus_id, ""),
        };

        // Raise the main terminal window
        os_helpers::wmctrl_focus(window_id)?;

        // If we have a shell PID, try to find and activate the specific tab
        if !shell_pid.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Use xdotool to search for windows owned by the shell PID
            if let Ok(output) = std::process::Command::new("xdotool")
                .args(["search", "--pid", shell_pid])
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for xid in stdout.lines() {
                    let xid = xid.trim();
                    if !xid.is_empty() {
                        let _ = os_helpers::xdotool_windowactivate(xid);
                        break;
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = focus_id;
        log::debug!("x11_generic focus is only supported on Linux");
    }

    Ok(())
}
