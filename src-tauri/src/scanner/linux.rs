use std::path::{Path, PathBuf};

use crate::watcher::TerminalInfo;
use super::{known_terminal, ProcInfo, WindowCache};

pub fn find_cli_processes() -> Vec<ProcInfo> {
    let mut out = Vec::new();
    let proc_dir = match std::fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => return out,
    };

    for entry in proc_dir.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let pid: u32 = match name_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let base = PathBuf::from("/proc").join(&*name_str);

        let cmdline = match std::fs::read(&base.join("cmdline")) {
            Ok(bytes) if !bytes.is_empty() => bytes,
            _ => continue,
        };

        let first_end = cmdline.iter().position(|&b| b == 0).unwrap_or(cmdline.len());
        let exe_path = String::from_utf8_lossy(&cmdline[..first_end]);
        let exe_name = Path::new(exe_path.as_ref())
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if exe_name != "claude" {
            continue;
        }

        let full = String::from_utf8_lossy(&cmdline);
        if full.contains("mcp-server") || full.contains("worker-service") {
            continue;
        }

        let stat = match std::fs::read_to_string(base.join("stat")) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let close = match stat.rfind(')') {
            Some(i) => i,
            None => continue,
        };
        let fields: Vec<&str> = stat[close + 2..].split_whitespace().collect();
        if fields.len() < 13 {
            continue;
        }

        let ppid: u32 = fields[1].parse().unwrap_or(0);
        let utime: u64 = fields[11].parse().unwrap_or(0);
        let stime: u64 = fields[12].parse().unwrap_or(0);

        let cwd = match std::fs::read_link(base.join("cwd")) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let tty_path = std::fs::read_link(base.join("fd/0"))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        if !tty_path.starts_with("/dev/pts/") && !tty_path.starts_with("/dev/tty") {
            continue;
        }

        let tty_label = tty_path
            .strip_prefix("/dev/")
            .unwrap_or(&tty_path)
            .to_string();

        out.push(ProcInfo {
            pid,
            ppid,
            cwd,
            tty_label,
            utime,
            stime,
            instant_cpu: None,
        });
    }

    out
}

pub fn terminal_info(cache: &mut WindowCache, p: &ProcInfo) -> Option<TerminalInfo> {
    let mut cur = p.ppid;
    let mut term_label = String::new();
    let mut term_pid: u32 = 0;

    for _ in 0..6 {
        if cur <= 1 {
            break;
        }
        let cmdline = std::fs::read_to_string(format!("/proc/{}/cmdline", cur)).ok()?;
        let exe = cmdline.split('\0').next().unwrap_or("");
        let exe_name = Path::new(exe)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if let Some(label) = known_terminal(exe_name) {
            term_label = label.to_string();
            term_pid = cur;
            break;
        }

        // Go up
        let stat = std::fs::read_to_string(format!("/proc/{}/stat", cur)).ok()?;
        let close = stat.rfind(')')?;
        let fields: Vec<&str> = stat[close + 2..].split_whitespace().collect();
        cur = fields.get(1)?.parse().ok()?;
    }

    if term_pid == 0 {
        return Some(TerminalInfo {
            kind: "x11_generic".to_string(),
            focus_id: String::new(),
            outer_id: String::new(),
            label: p.tty_label.clone(),
            window_title: None,
        });
    }

    // Skip xdotool on pure Wayland (no X11 display).
    // Known limitation: focus button is disabled on Wayland-only sessions.
    let has_display = std::env::var("DISPLAY").map_or(false, |v| !v.is_empty());

    let focus_id = if has_display {
        cache
            .entry(term_pid)
            .or_insert_with(|| xdotool_search_pid(term_pid))
            .clone()
            .unwrap_or_default()
    } else {
        String::new()
    };

    let window_title = if !focus_id.is_empty() {
        xdotool_get_name(&focus_id)
    } else {
        None
    };

    Some(TerminalInfo {
        kind: "x11_generic".to_string(),
        focus_id,
        outer_id: String::new(),
        label: term_label,
        window_title,
    })
}

fn xdotool_get_name(wid_hex: &str) -> Option<String> {
    let output = std::process::Command::new("xdotool")
        .args(["getwindowname", wid_hex])
        .output()
        .ok()?;
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() { None } else { Some(name) }
}

fn xdotool_search_pid(pid: u32) -> Option<String> {
    let output = std::process::Command::new("xdotool")
        .args(["search", "--pid", &pid.to_string()])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let decimal = stdout.lines().next()?.trim();
    if decimal.is_empty() {
        return None;
    }
    let wid: u64 = decimal.parse().ok()?;
    Some(format!("0x{:x}", wid))
}
