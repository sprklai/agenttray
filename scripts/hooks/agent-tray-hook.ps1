# agent-tray-hook.ps1 — Universal hook bridge for AgentTray (Windows)
# Called by Claude Code, Codex CLI, and Gemini CLI hook systems.
# Reads hook event JSON from stdin, maps it to AgentTray status,
# and writes atomic status files to ~/.agent-monitor/.

$ErrorActionPreference = 'SilentlyContinue'

$MonitorDir = Join-Path $env:USERPROFILE '.agent-monitor'
if (-not (Test-Path $MonitorDir)) {
    New-Item -ItemType Directory -Path $MonitorDir -Force | Out-Null
}

# ── Read stdin FIRST (hook systems pipe JSON) ─────────────────
# Must happen before CLI/session detection since those need the JSON.

$InputText = ''
if ([Console]::IsInputRedirected) {
    $InputText = [Console]::In.ReadToEnd()
}

$InputObj = $null
if ($InputText) {
    try { $InputObj = $InputText | ConvertFrom-Json } catch {}
}

# ── CLI Detection ──────────────────────────────────────────────

function Get-CliName {
    # Check CLI-specific env vars first (most specific indicators).
    # session_id in JSON is a common field for ALL CLIs, not Claude-specific.
    if ($env:GEMINI_CLI -eq '1' -or $env:GEMINI_SESSION_ID) { return 'gemini' }
    if ($env:CODEX_SESSION_ID) { return 'codex' }
    if ($env:CLAUDE_SESSION_ID) { return 'claude-code' }

    # Fallback: check parent process name
    try {
        $parent = (Get-Process -Id $PID).Parent
        if ($parent) {
            $pname = $parent.ProcessName.ToLower()
            if ($pname -match 'claude') { return 'claude-code' }
            if ($pname -match 'codex')  { return 'codex' }
            if ($pname -match 'gemini') { return 'gemini' }
        }
    } catch {}

    return 'unknown'
}

$Cli = Get-CliName

# ── Session ID ─────────────────────────────────────────────────

function Get-SessionId {
    param([string]$CliName, [object]$JsonInput)
    # Prefer session_id from JSON input (most reliable in hook subprocesses
    # where env vars like CLAUDE_SESSION_ID may not be available)
    if ($JsonInput -and $JsonInput.session_id) {
        return $JsonInput.session_id
    }
    # Fallback to env vars
    switch ($CliName) {
        'claude-code' { if ($env:CLAUDE_SESSION_ID) { return $env:CLAUDE_SESSION_ID } }
        'codex'       { if ($env:CODEX_SESSION_ID)  { return $env:CODEX_SESSION_ID } }
        'gemini'      { if ($env:GEMINI_SESSION_ID)  { return $env:GEMINI_SESSION_ID } }
    }
    return "$PID"
}

$SessionId = Get-SessionId -CliName $Cli -JsonInput $InputObj
$SessionShort = $SessionId.Substring(0, [Math]::Min(8, $SessionId.Length))
$StatusFile = Join-Path $MonitorDir "${Cli}-${SessionShort}.status"

# ── Terminal Info ──────────────────────────────────────────────

function Get-TerminalJson {
    $kind = 'windows_native'
    $focusId = ''
    $outerId = ''
    $label = 'Windows Terminal'
    $windowTitle = ''

    try {
        $parentPid = (Get-Process -Id $PID).Parent.Id
        if ($parentPid) { $focusId = "$parentPid" }
        $parent = Get-Process -Id $parentPid -ErrorAction SilentlyContinue
        if ($parent) {
            if ($parent.ProcessName -match 'WindowsTerminal') { $label = 'Windows Terminal' }
            elseif ($parent.ProcessName -match 'powershell|pwsh') { $label = 'PowerShell' }
            elseif ($parent.ProcessName -match 'cmd') { $label = 'CMD' }
            elseif ($parent.ProcessName -match 'alacritty') { $label = 'Alacritty' }
            elseif ($parent.ProcessName -match 'wezterm') { $label = 'WezTerm' }
        }
    } catch {}

    if ($env:WT_SESSION) { $label = 'Windows Terminal' }
    if ($env:ConEmuPID)  { $label = 'ConEmu' }

    return "{`"kind`":`"$kind`",`"focus_id`":`"$focusId`",`"outer_id`":`"$outerId`",`"label`":`"$label`",`"window_title`":`"$windowTitle`"}"
}

$TerminalJson = Get-TerminalJson

# ── JSON Helpers ───────────────────────────────────────────────

function ConvertTo-JsonString {
    param([string]$Value)
    $Value = $Value -replace '\\', '\\\\' -replace '"', '\"' -replace "`n", '\n' -replace "`r", '\r' -replace "`t", '\t'
    return "`"$Value`""
}

# ── Status File Writer ─────────────────────────────────────────

function Write-Status {
    param([string]$Status, [string]$Message, [string]$HookEvent, [string]$HookMatcher = '')
    if ($Message.Length -gt 500) { $Message = $Message.Substring(0, 500) }
    $safeMsg = ConvertTo-JsonString -Value $Message

    $json = "{`"v`":1,`"status`":`"$Status`",`"message`":$safeMsg,`"source`":`"hook`",`"cli`":`"$Cli`",`"session_id`":`"$SessionId`",`"hook_event`":`"$HookEvent`",`"hook_matcher`":`"$HookMatcher`",`"terminal`":$TerminalJson}"

    $tmpFile = "$StatusFile.tmp"
    [System.IO.File]::WriteAllText($tmpFile, $json)
    Move-Item -Path $tmpFile -Destination $StatusFile -Force
}

function Remove-StatusFile {
    Remove-Item -Path $StatusFile -Force -ErrorAction SilentlyContinue
    Remove-Item -Path "$StatusFile.tmp" -Force -ErrorAction SilentlyContinue
}

# ── Event Mapping ──────────────────────────────────────────────

# Extract event name (try multiple field names)
$Event = ''
if ($InputObj) {
    $Event = ($InputObj.hook_event_name, $InputObj.event, $InputObj.type, $InputObj.hook_name |
              Where-Object { $_ } | Select-Object -First 1)
    if (-not $Event) { $Event = '' }
}

# Get matcher / subtype
$Matcher = ''
if ($InputObj) {
    $Matcher = ($InputObj.notification_type, $InputObj.tool_name |
                Where-Object { $_ } | Select-Object -First 1)
    if (-not $Matcher) { $Matcher = '' }
}

# ── Claude Code Mapping ───────────────────────────────────────

function Map-ClaudeCode {
    switch ($Event) {
        'SessionStart' {
            Write-Status -Status 'working' -Message 'Session started' -HookEvent $Event
        }
        'Notification' {
            switch ($Matcher) {
                'permission_prompt'   { Write-Status -Status 'needs-input' -Message 'Waiting for permission' -HookEvent $Event -HookMatcher $Matcher }
                'idle_prompt'         { Write-Status -Status 'needs-input' -Message 'Waiting for input' -HookEvent $Event -HookMatcher $Matcher }
                'elicitation_dialog'  { Write-Status -Status 'needs-input' -Message 'MCP input requested' -HookEvent $Event -HookMatcher $Matcher }
                default               { Write-Status -Status 'needs-input' -Message "Notification: $Matcher" -HookEvent $Event -HookMatcher $Matcher }
            }
        }
        'Stop' {
            Write-Status -Status 'needs-input' -Message 'Waiting for input' -HookEvent $Event
        }
        'StopFailure' {
            Write-Status -Status 'error' -Message 'API error' -HookEvent $Event
        }
        'UserPromptSubmit' {
            Write-Status -Status 'working' -Message 'Processing prompt' -HookEvent $Event
        }
        'PreToolUse' {
            switch ($Matcher) {
                'Bash'       { Write-Status -Status 'working' -Message 'Running command...' -HookEvent $Event -HookMatcher $Matcher }
                { $_ -in 'Edit','Write' } { Write-Status -Status 'working' -Message 'Editing files...' -HookEvent $Event -HookMatcher $Matcher }
                'Agent'      { Write-Status -Status 'working' -Message 'Running subagent...' -HookEvent $Event -HookMatcher $Matcher }
                default      { Write-Status -Status 'working' -Message "Using ${Matcher}..." -HookEvent $Event -HookMatcher $Matcher }
            }
        }
        'PostToolUse' {
            Write-Status -Status 'working' -Message 'Tool completed' -HookEvent $Event -HookMatcher $Matcher
        }
        'SubagentStop' {
            Write-Status -Status 'working' -Message 'Subagent completed' -HookEvent $Event
        }
        'SessionEnd' {
            Remove-StatusFile
        }
    }
}

# ── Codex CLI Mapping ─────────────────────────────────────────

function Map-Codex {
    switch ($Event) {
        'SessionStart'     { Write-Status -Status 'working' -Message 'Session started' -HookEvent $Event }
        'PreToolUse'       { Write-Status -Status 'working' -Message 'Running tool...' -HookEvent $Event -HookMatcher $Matcher }
        'PostToolUse'      { Write-Status -Status 'working' -Message 'Tool completed' -HookEvent $Event -HookMatcher $Matcher }
        'Stop'             { Write-Status -Status 'idle' -Message 'Finished responding' -HookEvent $Event }
        'UserPromptSubmit' { Write-Status -Status 'working' -Message 'Processing prompt' -HookEvent $Event }
        'SessionEnd'       { Remove-StatusFile }
    }
}

# ── Gemini CLI Mapping ────────────────────────────────────────

function Map-Gemini {
    switch ($Event) {
        'SessionStart' { Write-Status -Status 'working' -Message 'Session started' -HookEvent $Event }
        'SessionEnd'   { Remove-StatusFile }
        'BeforeAgent'  { Write-Status -Status 'working' -Message 'Processing...' -HookEvent $Event }
        'AfterAgent'   { Write-Status -Status 'idle' -Message 'Finished responding' -HookEvent $Event }
        'BeforeTool' {
            $toolLabel = if ($Matcher) { $Matcher } else { 'tool' }
            Write-Status -Status 'working' -Message "Running tool: $toolLabel" -HookEvent $Event -HookMatcher $Matcher
        }
        'AfterTool' {
            $toolLabel = if ($Matcher) { $Matcher } else { 'tool' }
            Write-Status -Status 'working' -Message "Tool completed: $toolLabel" -HookEvent $Event -HookMatcher $Matcher
        }
        'Notification' { Write-Status -Status 'needs-input' -Message 'Waiting for input' -HookEvent $Event -HookMatcher $Matcher }
        'PreCompress'  { Write-Status -Status 'working' -Message 'Compacting context' -HookEvent $Event }
    }
}

# ── Dispatch ──────────────────────────────────────────────────

switch ($Cli) {
    'claude-code' { Map-ClaudeCode }
    'codex'       { Map-Codex }
    'gemini'      { Map-Gemini }
    default {
        # Unknown CLI — try to map generic events
        switch ($Event) {
            'SessionStart'              { Write-Status -Status 'working' -Message 'Session started' -HookEvent $Event }
            'SessionEnd'                { Remove-StatusFile }
            'Stop'                      { Write-Status -Status 'idle' -Message 'Finished responding' -HookEvent $Event }
            'Notification'              { Write-Status -Status 'needs-input' -Message 'Notification' -HookEvent $Event -HookMatcher $Matcher }
            { $_ -in 'PreToolUse','BeforeTool' } { Write-Status -Status 'working' -Message 'Running tool...' -HookEvent $Event -HookMatcher $Matcher }
        }
    }
}

# Hook scripts must exit 0 to not block the CLI
exit 0
