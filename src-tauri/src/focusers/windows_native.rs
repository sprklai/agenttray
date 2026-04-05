/// Activates a Windows terminal window using PowerShell and Win32 SetForegroundWindow.
/// `focus_id` is the terminal process ID as a string.

pub fn focus(focus_id: &str, _outer_id: &str) -> Result<(), String> {
    if focus_id.is_empty() {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        // Use PowerShell to get the main window handle and bring it to front
        let script = format!(
            r#"
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win32 {{
    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
}}
"@
$p = Get-Process -Id {} -ErrorAction SilentlyContinue
if ($p -and $p.MainWindowHandle -ne [IntPtr]::Zero) {{
    [Win32]::ShowWindow($p.MainWindowHandle, 9)  # SW_RESTORE
    [Win32]::SetForegroundWindow($p.MainWindowHandle)
}}
"#,
            focus_id
        );

        super::os_helpers::spawn_silent(
            "powershell",
            &["-NoProfile", "-NonInteractive", "-Command", &script],
        )?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = focus_id;
        log::debug!("windows_native focus is only supported on Windows");
    }

    Ok(())
}
