# wrap.ps1 <agent-name> <command> [args...]
# Wraps an AI agent command and writes real-time status to ~/.agent-monitor/
# PowerShell equivalent of wrap.sh for Windows users.

param(
    [Parameter(Mandatory=$true, Position=0)]
    [string]$AgentName,

    [Parameter(Mandatory=$true, Position=1, ValueFromRemainingArguments=$true)]
    [string[]]$Command
)

$ErrorActionPreference = 'Stop'

$MonitorDir = Join-Path $env:USERPROFILE '.agent-monitor'
$StatusFile = Join-Path $MonitorDir "$AgentName.status"

if (-not (Test-Path $MonitorDir)) {
    New-Item -ItemType Directory -Path $MonitorDir -Force | Out-Null
}

# Detect terminal
$TerminalJson = $null
$RegistryScript = Join-Path $PSScriptRoot 'registry.ps1'
if (Test-Path $RegistryScript) {
    try { $TerminalJson = & $RegistryScript 2>$null } catch {}
}
if (-not $TerminalJson) {
    $parentPid = (Get-Process -Id $PID).Parent.Id
    $termApp = 'Windows Terminal'
    try {
        $parent = Get-Process -Id $parentPid -ErrorAction SilentlyContinue
        if ($parent.ProcessName -match 'WindowsTerminal') { $termApp = 'Windows Terminal' }
        elseif ($parent.ProcessName -match 'powershell|pwsh') { $termApp = 'PowerShell' }
        elseif ($parent.ProcessName -match 'cmd') { $termApp = 'CMD' }
    } catch {}
    $TerminalJson = "{`"kind`":`"windows_native`",`"focus_id`":`"$parentPid`",`"outer_id`":`"`",`"label`":`"$termApp`"}"
}

# Input detection patterns
$InputPatterns = @(
    '\? $', '\? .*\[', '\[y/n\]', '\[Y/n\]', '\[yes/no\]',
    'password:', 'Password:', 'passphrase:', 'Passphrase:',
    'Enter to', 'Press .* to', 'Overwrite\?', 'Continue\?',
    'Confirm\?', 'Proceed\?', 'Are you sure'
)
$InputRegex = ($InputPatterns -join '|')

function Write-Status {
    param([string]$Status, [string]$Message)
    if ($Message.Length -gt 500) { $Message = $Message.Substring(0, 500) }
    # Escape JSON special characters
    $SafeMessage = $Message -replace '\\', '\\\\' -replace '"', '\"' -replace "`n", '\n' -replace "`r", '\r' -replace "`t", '\t'
    $Json = "{`"v`":1,`"status`":`"$Status`",`"message`":`"$SafeMessage`",`"terminal`":$TerminalJson}"
    $TmpFile = "$StatusFile.tmp"
    [System.IO.File]::WriteAllText($TmpFile, $Json)
    Move-Item -Path $TmpFile -Destination $StatusFile -Force
}

function Remove-StatusFile {
    Remove-Item -Path $StatusFile -Force -ErrorAction SilentlyContinue
    Remove-Item -Path "$StatusFile.tmp" -Force -ErrorAction SilentlyContinue
}

# Cleanup on exit
$null = Register-EngineEvent -SourceIdentifier PowerShell.Exiting -Action { Remove-StatusFile }
try {
    # Write starting status
    Write-Status -Status 'starting' -Message ''

    # Run the command and capture output line by line
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo.FileName = $Command[0]
    if ($Command.Length -gt 1) {
        $process.StartInfo.Arguments = ($Command[1..($Command.Length-1)] -join ' ')
    }
    $process.StartInfo.UseShellExecute = $false
    $process.StartInfo.RedirectStandardOutput = $true
    $process.StartInfo.RedirectStandardError = $true
    $process.StartInfo.CreateNoWindow = $true

    $process.Start() | Out-Null

    # Read stdout and stderr merged
    while (-not $process.HasExited -or -not $process.StandardOutput.EndOfStream) {
        $line = $process.StandardOutput.ReadLine()
        if ($null -eq $line) { continue }

        Write-Host $line

        if ($line -match $InputRegex) {
            Write-Status -Status 'needs-input' -Message $line
        } else {
            Write-Status -Status 'working' -Message $line
        }
    }

    # Also drain stderr
    $stderr = $process.StandardError.ReadToEnd()
    if ($stderr) { Write-Host $stderr -ForegroundColor Red }

    $process.WaitForExit()

    if ($process.ExitCode -eq 0) {
        Remove-StatusFile
    } else {
        Write-Status -Status 'error' -Message "Exit code $($process.ExitCode)"
    }
} catch {
    Write-Status -Status 'error' -Message $_.Exception.Message
} finally {
    # Ensure cleanup on Ctrl+C
    if ($process -and -not $process.HasExited) {
        $process.Kill()
    }
}
