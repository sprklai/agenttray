use std::path::{Path, PathBuf};

use crate::watcher::TerminalInfo;
use super::{known_terminal, ProcInfo, WindowCache};
use super::strategies::{CliStrategy, SCRIPT_RUNTIMES};

/// Find CLI processes matching any registered strategy.
pub fn find_cli_processes<'a>(
    strategies: &'a [Box<dyn CliStrategy>],
) -> Vec<(ProcInfo, &'a dyn CliStrategy)> {
    // Build wmic filter for all strategy process names + script runtimes
    let all_names: Vec<&str> = strategies.iter()
        .flat_map(|s| s.process_names().iter().copied())
        .chain(SCRIPT_RUNTIMES.iter().copied())
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

        // Phase 1: Direct process name match (native binaries)
        let strategy = strategies.iter().find(|s| {
            s.process_names().iter().any(|n| *n == proc_name)
        });

        // Phase 2: If process is a script runtime, check CommandLine for script names
        let strategy = match strategy {
            Some(s) => s.as_ref(),
            None => {
                if !SCRIPT_RUNTIMES.iter().any(|r| *r == proc_name) {
                    continue;
                }
                match cmd_line.split_whitespace().skip(1).find_map(|arg| {
                    if arg.starts_with('-') {
                        return None;
                    }
                    let arg_name = Path::new(arg)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");
                    strategies.iter().find(|s| {
                        s.script_names().iter().any(|n| *n == arg_name)
                    })
                }) {
                    Some(s) => s.as_ref(),
                    None => continue,
                }
            }
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

/// Read a single environment variable from a running process on Windows.
///
/// Uses PowerShell `Add-Type` to PInvoke the Windows API:
/// `OpenProcess` → `NtQueryInformationProcess` (PEB address) →
/// `ReadProcessMemory` (ProcessParameters → Environment block) → parse.
///
/// Works for same-user processes. Requires PROCESS_QUERY_INFORMATION |
/// PROCESS_VM_READ access (both granted by default for same-user processes).
/// The result is cached in Scanner.session_id_cache, so PowerShell is only
/// spawned once per PID across scan cycles.
pub fn read_proc_env(pid: u32, key: &str) -> Option<String> {
    // Guard against injection — env var names are always safe ASCII identifiers.
    if !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return None;
    }
    let script = PS_READ_ENV
        .replace("__PID__", &pid.to_string())
        .replace("__KEY__", key);
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .ok()?;
    let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if val.is_empty() { None } else { Some(val) }
}

/// PowerShell script that reads a process environment variable via Windows APIs.
/// __PID__ and __KEY__ are substituted at runtime.
const PS_READ_ENV: &str = r#"
$p = __PID__; $k = '__KEY__'
if (-not ([System.Management.Automation.PSTypeName]'AgentTrayEnv').Type) {
    Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices; using System.Text;
public class AgentTrayEnv {
    const int PROCESS_QUERY_INFORMATION = 0x0400, PROCESS_VM_READ = 0x0010;
    [DllImport("kernel32")] static extern IntPtr OpenProcess(int a, bool b, int c);
    [DllImport("kernel32")] static extern bool ReadProcessMemory(IntPtr h, IntPtr addr, byte[] buf, int sz, out int n);
    [DllImport("kernel32")] static extern bool CloseHandle(IntPtr h);
    [DllImport("ntdll")] static extern int NtQueryInformationProcess(IntPtr h, int cls, ref PBI pbi, int sz, out int ret);
    [StructLayout(LayoutKind.Sequential)] struct PBI {
        IntPtr r1; public IntPtr PebBase; IntPtr r2, r3; UIntPtr r4; IntPtr r5;
    }
    static IntPtr ReadPtr(IntPtr h, IntPtr addr) {
        var b = new byte[IntPtr.Size]; int n;
        ReadProcessMemory(h, addr, b, b.Length, out n);
        return IntPtr.Size == 8
            ? new IntPtr(BitConverter.ToInt64(b, 0))
            : new IntPtr(BitConverter.ToInt32(b, 0));
    }
    public static string Get(int pid, string key) {
        var h = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid);
        if (h == IntPtr.Zero) return null;
        try {
            var pbi = new PBI(); int ret;
            NtQueryInformationProcess(h, 0, ref pbi, Marshal.SizeOf(typeof(PBI)), out ret);
            bool x64 = IntPtr.Size == 8;
            var pp = ReadPtr(h, IntPtr.Add(pbi.PebBase, x64 ? 0x20 : 0x10));
            var ep = ReadPtr(h, IntPtr.Add(pp, x64 ? 0x80 : 0x48));
            var buf = new byte[65536]; int read;
            ReadProcessMemory(h, ep, buf, buf.Length, out read);
            var env = Encoding.Unicode.GetString(buf, 0, Math.Max(0, read - 2));
            foreach (var pair in env.Split('\0'))
                if (pair.StartsWith(key + "=", StringComparison.OrdinalIgnoreCase))
                    return pair.Substring(key.Length + 1);
            return null;
        } finally { CloseHandle(h); }
    }
}
'@ -ErrorAction SilentlyContinue
}
$r = [AgentTrayEnv]::Get($p, $k)
if ($r) { Write-Output $r }
"#;
