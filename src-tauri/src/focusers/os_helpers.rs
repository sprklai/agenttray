use std::process::Command;

pub fn spawn_silent(cmd: &str, args: &[&str]) -> Result<(), String> {
    match Command::new(cmd).args(args).output() {
        Ok(output) => {
            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("{} failed: {}", cmd, stderr.trim()))
            }
        }
        Err(e) => {
            // Tool not installed — not an error for the user
            log::debug!("{} not available: {}", cmd, e);
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
pub fn wmctrl_focus(window_id: &str) -> Result<(), String> {
    spawn_silent("wmctrl", &["-ia", window_id])
}

#[cfg(target_os = "linux")]
pub fn xdotool_windowactivate(window_id: &str) -> Result<(), String> {
    spawn_silent("xdotool", &["windowactivate", window_id])
}
