/// Focus a JetBrains IDE terminal window.
/// Falls back to platform-specific window activation.

pub fn focus(_focus_id: &str, _outer_id: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use super::os_helpers::spawn_silent;

        // Try common JetBrains IDE app names
        let ide_names = [
            "IntelliJ IDEA",
            "PyCharm",
            "WebStorm",
            "GoLand",
            "CLion",
            "PhpStorm",
            "RubyMine",
            "Rider",
            "DataGrip",
            "Android Studio",
        ];

        // Find which IDE is running by checking running processes
        let output = std::process::Command::new("osascript")
            .args([
                "-e",
                r#"tell application "System Events" to get name of every process whose background only is false"#,
            ])
            .output();

        if let Ok(output) = output {
            let running = String::from_utf8_lossy(&output.stdout);
            for name in &ide_names {
                if running.contains(name) {
                    let script = format!("tell application \"{}\" to activate", name);
                    return spawn_silent("osascript", &["-e", &script]);
                }
            }
        }

        // Last resort: try activating the generic process name
        return Err("Could not identify running JetBrains IDE".to_string());
    }

    #[cfg(target_os = "linux")]
    {
        // Try wmctrl with JetBrains window class patterns
        let classes = ["jetbrains-idea", "jetbrains-pycharm", "jetbrains-webstorm",
                       "jetbrains-goland", "jetbrains-clion", "jetbrains-phpstorm",
                       "jetbrains-rubymine", "jetbrains-rider", "jetbrains-datagrip"];

        for class in &classes {
            if super::os_helpers::spawn_silent("wmctrl", &["-xa", class]).is_ok() {
                return Ok(());
            }
        }
        Err("Could not find JetBrains IDE window".to_string())
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows, use the windows_native focuser with the PPID
        let _ = (_focus_id, _outer_id);
        Err("JetBrains focus on Windows: use windows_native kind instead".to_string())
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (_focus_id, _outer_id);
        Err("JetBrains focus not supported on this platform".to_string())
    }
}
