use std::path::PathBuf;

use crate::watcher::TerminalInfo;
use super::{known_terminal, ProcInfo, WindowCache};
use super::strategies::CliStrategy;

/// Find CLI processes matching any registered strategy.
pub fn find_cli_processes<'a>(
    strategies: &'a [Box<dyn CliStrategy>],
) -> Vec<(ProcInfo, &'a dyn CliStrategy)> {
    // Build wmic filter for all strategy process names
    let all_names: Vec<&str> = strategies.iter()
        .flat_map(|s| s.process_names().iter().copied())
        .collect();
    if all_names.is_empty() {
        return Vec::new();
    }

    // Query all potential agent processes in one wmic call
    let name_filter = all_names.iter()
        .map(|n| format!("name='{}.exe'", n))
        .collect::<Vec<_>>()
        .join(" or ");

    let output = match std::process::Command::new("wmic")
        .args([
            "process",
            "where",
            &name_filter,
            "get",
            "ProcessId,ParentProcessId,CommandLine,Name",
            "/FORMAT:CSV",
        ])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut out = Vec::new();

    for line in stdout.lines().skip(1) {
        let cols: Vec<&str> = line.split(',').collect();
        // CSV format: Node,CommandLine,Name,ParentProcessId,ProcessId
        if cols.len() < 5 {
            continue;
        }

        let cmd_line = cols[1];
        let proc_name = cols[2].trim().strip_suffix(".exe").unwrap_or(cols[2].trim());

        let strategy = match strategies.iter().find(|s| {
            s.process_names().iter().any(|n| *n == proc_name)
        }) {
            Some(s) => s.as_ref(),
            None => continue,
        };

        if strategy.excluded_substrings().iter().any(|exc| cmd_line.contains(exc)) {
            continue;
        }

        let pid: u32 = match cols[4].trim().parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let ppid: u32 = cols[3].trim().parse().unwrap_or(0);

        // CWD is hard to get on Windows without elevated privileges
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("C:\\"));

        // Get CPU times (100-nanosecond units) via wmic
        let (utime, stime) = wmic_cpu_times(pid);

        out.push((ProcInfo {
            pid,
            ppid,
            cwd,
            tty_label: String::new(),
            utime,
            stime,
            instant_cpu: None,
            window_title: None,
            last_active: None,
        }, strategy));
    }

    out
}

pub fn terminal_info(cache: &mut WindowCache, p: &ProcInfo) -> Option<TerminalInfo> {
    let mut cur = p.ppid;
    let mut term_label = String::new();
    let mut term_pid: u32 = 0;

    // Walk parent process chain to find the terminal emulator
    for _ in 0..6 {
        if cur <= 1 {
            break;
        }

        let output = std::process::Command::new("wmic")
            .args([
                "process",
                "where",
                &format!("ProcessId={}", cur),
                "get",
                "ParentProcessId,Name",
                "/FORMAT:CSV",
            ])
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        // CSV: Node,Name,ParentProcessId
        let line = stdout.lines().find(|l| !l.trim().is_empty() && !l.contains("Node"))?;
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() < 3 {
            break;
        }

        let exe_name = cols[1].trim();
        if let Some(label) = known_terminal(exe_name) {
            term_label = label.to_string();
            term_pid = cur;
            break;
        }

        cur = cols[2].trim().parse().unwrap_or(0);
    }

    if term_pid == 0 {
        return None;
    }

    let window_title = cache
        .entry(term_pid)
        .or_insert_with(|| powershell_window_title(term_pid))
        .clone();

    Some(TerminalInfo {
        kind: "windows_native".to_string(),
        focus_id: term_pid.to_string(),
        outer_id: String::new(),
        label: term_label,
        window_title,
    })
}

/// Returns (user_time, kernel_time) in 100-nanosecond units.
/// We convert to CLK_TCK-equivalent ticks (divide by 100_000 to get ~10ms ticks)
/// so the Scanner::cpu_pct delta math works the same as Linux.
fn wmic_cpu_times(pid: u32) -> (u64, u64) {
    let output = match std::process::Command::new("wmic")
        .args([
            "process",
            "where",
            &format!("ProcessId={}", pid),
            "get",
            "UserModeTime,KernelModeTime",
            "/FORMAT:CSV",
        ])
        .output()
    {
        Ok(o) => o,
        Err(_) => return (0, 0),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    // CSV: Node,KernelModeTime,UserModeTime
    for line in stdout.lines().skip(1) {
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() >= 3 {
            let kernel: u64 = cols[1].trim().parse().unwrap_or(0);
            let user: u64 = cols[2].trim().parse().unwrap_or(0);
            // Convert 100ns units to ~10ms ticks (CLK_TCK=100 equivalent)
            return (user / 100_000, kernel / 100_000);
        }
    }
    (0, 0)
}

fn powershell_window_title(pid: u32) -> Option<String> {
    let cmd = format!("(Get-Process -Id {}).MainWindowTitle", pid);
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &cmd])
        .output()
        .ok()?;
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() { None } else { Some(name) }
}
