# install-hooks.ps1 — Install/uninstall AgentTray hooks for AI CLI tools (Windows)
# Usage: install-hooks.ps1 [-Target claude|codex|gemini|all] [-Uninstall]

param(
    [ValidateSet('claude', 'codex', 'gemini', 'all')]
    [string]$Target = 'all',
    [switch]$Uninstall
)

$ErrorActionPreference = 'Stop'

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$HookScriptSrc = Join-Path $ScriptDir 'agent-tray-hook.ps1'
$HookInstallDir = Join-Path $env:USERPROFILE '.agent-monitor\hooks'
$InstalledHook = Join-Path $HookInstallDir 'agent-tray-hook.ps1'
$HookCommand = "powershell -NoProfile -ExecutionPolicy Bypass -File `"$InstalledHook`""
$HookTag = 'agent-tray'

# ── Prerequisites ─────────────────────────────────────────────

if (-not (Test-Path $HookScriptSrc)) {
    Write-Error "Hook script not found: $HookScriptSrc"
    exit 1
}

# ── Helpers ───────────────────────────────────────────────────

function Ensure-JsonFile {
    param([string]$Path)
    $dir = Split-Path -Parent $Path
    if (-not (Test-Path $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    if (-not (Test-Path $Path)) {
        '{}' | Set-Content -Path $Path -Encoding UTF8
    }
}

function Read-JsonFile {
    param([string]$Path)
    $raw = Get-Content -Path $Path -Raw -Encoding UTF8
    if (-not $raw -or $raw.Trim() -eq '') { $raw = '{}' }
    return $raw | ConvertFrom-Json
}

function Write-JsonFile {
    param([string]$Path, [object]$Data)
    $json = $Data | ConvertTo-Json -Depth 10
    $tmpFile = "$Path.tmp"
    [System.IO.File]::WriteAllText($tmpFile, $json, [System.Text.Encoding]::UTF8)
    Move-Item -Path $tmpFile -Destination $Path -Force
}

function Deploy-HookScripts {
    if (-not (Test-Path $HookInstallDir)) {
        New-Item -ItemType Directory -Path $HookInstallDir -Force | Out-Null
    }
    Copy-Item -Path $HookScriptSrc -Destination $InstalledHook -Force
    Write-Host "  -> Deployed hook script to $InstalledHook"

    # Also deploy bash version if present (for Git Bash users)
    $bashSrc = Join-Path $ScriptDir 'agent-tray-hook.sh'
    if (Test-Path $bashSrc) {
        $bashDest = Join-Path $HookInstallDir 'agent-tray-hook.sh'
        Copy-Item -Path $bashSrc -Destination $bashDest -Force
        Write-Host "  -> Deployed bash hook to $bashDest"
    }
}

# Remove agent-tray hooks from a nested matcher+hooks array (Claude Code format)
function Remove-AgentTrayNested {
    param([array]$EventEntries)
    $result = @()
    foreach ($entry in $EventEntries) {
        if ($entry.hooks) {
            # New matcher+hooks format
            $filtered = @($entry.hooks | Where-Object { $_.tag -ne $HookTag })
            if ($filtered.Count -gt 0) {
                $entry.hooks = $filtered
                $result += $entry
            }
        } elseif ($entry.tag -ne $HookTag) {
            # Legacy flat format
            $result += $entry
        }
    }
    return $result
}

# Remove agent-tray hooks from a flat array (Codex/Gemini format)
function Remove-AgentTrayFlat {
    param([array]$EventEntries)
    return @($EventEntries | Where-Object { $_.tag -ne $HookTag })
}

# ── Claude Code ───────────────────────────────────────────────

$ClaudeSettings = Join-Path $env:USERPROFILE '.claude\settings.json'
$ClaudeEvents = @('SessionStart', 'SessionEnd', 'Notification', 'Stop', 'StopFailure',
                   'UserPromptSubmit', 'PreToolUse', 'PostToolUse', 'SubagentStop')

function Install-Claude {
    Write-Host 'Installing AgentTray hooks for Claude Code...'
    Ensure-JsonFile -Path $ClaudeSettings
    $settings = Read-JsonFile -Path $ClaudeSettings

    # Ensure .hooks exists as a PSCustomObject
    if (-not $settings.hooks) {
        $settings | Add-Member -NotePropertyName 'hooks' -NotePropertyValue ([PSCustomObject]@{}) -Force
    }

    foreach ($event in $ClaudeEvents) {
        # Get existing entries and remove agent-tray
        $existing = @()
        if ($settings.hooks.PSObject.Properties[$event]) {
            $existing = Remove-AgentTrayNested -EventEntries @($settings.hooks.$event)
        }

        # Build new matcher+hooks entry (Claude Code format)
        $newEntry = [PSCustomObject]@{
            matcher = ''
            hooks = @(
                [PSCustomObject]@{ type = 'command'; command = $HookCommand; tag = $HookTag }
            )
        }

        $existing += $newEntry

        if ($settings.hooks.PSObject.Properties[$event]) {
            $settings.hooks.$event = $existing
        } else {
            $settings.hooks | Add-Member -NotePropertyName $event -NotePropertyValue $existing -Force
        }
    }

    Write-JsonFile -Path $ClaudeSettings -Data $settings
    Write-Host "  -> Updated $ClaudeSettings"
}

function Uninstall-Claude {
    Write-Host 'Removing AgentTray hooks from Claude Code...'
    if (-not (Test-Path $ClaudeSettings)) {
        Write-Host '  -> No settings file found, nothing to do.'
        return
    }

    $settings = Read-JsonFile -Path $ClaudeSettings
    if (-not $settings.hooks) { return }

    $propsToRemove = @()
    foreach ($prop in $settings.hooks.PSObject.Properties) {
        $cleaned = Remove-AgentTrayNested -EventEntries @($prop.Value)
        if ($cleaned.Count -eq 0) {
            $propsToRemove += $prop.Name
        } else {
            $settings.hooks.($prop.Name) = $cleaned
        }
    }
    foreach ($name in $propsToRemove) {
        $settings.hooks.PSObject.Properties.Remove($name)
    }

    # Remove hooks key if empty
    if (($settings.hooks.PSObject.Properties | Measure-Object).Count -eq 0) {
        $settings.PSObject.Properties.Remove('hooks')
    }

    Write-JsonFile -Path $ClaudeSettings -Data $settings
    Write-Host "  -> Cleaned $ClaudeSettings"
}

# ── Codex CLI ─────────────────────────────────────────────────

$CodexSettings = Join-Path $env:USERPROFILE '.codex\hooks.json'
$CodexEvents = @('SessionStart', 'PreToolUse', 'PostToolUse', 'Stop', 'UserPromptSubmit')

function Install-Codex {
    Write-Host 'Installing AgentTray hooks for Codex CLI...'
    Ensure-JsonFile -Path $CodexSettings
    $settings = Read-JsonFile -Path $CodexSettings

    if (-not $settings.hooks) {
        $settings | Add-Member -NotePropertyName 'hooks' -NotePropertyValue ([PSCustomObject]@{}) -Force
    }

    foreach ($event in $CodexEvents) {
        $existing = @()
        if ($settings.hooks.PSObject.Properties[$event]) {
            $existing = Remove-AgentTrayFlat -EventEntries @($settings.hooks.$event)
        }

        $newEntry = [PSCustomObject]@{ type = 'command'; command = $HookCommand; tag = $HookTag }
        $existing += $newEntry

        if ($settings.hooks.PSObject.Properties[$event]) {
            $settings.hooks.$event = $existing
        } else {
            $settings.hooks | Add-Member -NotePropertyName $event -NotePropertyValue $existing -Force
        }
    }

    Write-JsonFile -Path $CodexSettings -Data $settings
    Write-Host "  -> Updated $CodexSettings"
}

function Uninstall-Codex {
    Write-Host 'Removing AgentTray hooks from Codex CLI...'
    if (-not (Test-Path $CodexSettings)) {
        Write-Host '  -> No settings file found, nothing to do.'
        return
    }

    $settings = Read-JsonFile -Path $CodexSettings
    if (-not $settings.hooks) { return }

    $propsToRemove = @()
    foreach ($prop in $settings.hooks.PSObject.Properties) {
        $cleaned = Remove-AgentTrayFlat -EventEntries @($prop.Value)
        if ($cleaned.Count -eq 0) {
            $propsToRemove += $prop.Name
        } else {
            $settings.hooks.($prop.Name) = $cleaned
        }
    }
    foreach ($name in $propsToRemove) {
        $settings.hooks.PSObject.Properties.Remove($name)
    }

    if (($settings.hooks.PSObject.Properties | Measure-Object).Count -eq 0) {
        $settings.PSObject.Properties.Remove('hooks')
    }

    Write-JsonFile -Path $CodexSettings -Data $settings
    Write-Host "  -> Cleaned $CodexSettings"
}

# ── Gemini CLI ────────────────────────────────────────────────

$GeminiSettings = Join-Path $env:USERPROFILE '.gemini\settings.json'
$GeminiEvents = @('SessionStart', 'SessionEnd', 'BeforeAgent', 'AfterAgent',
                   'BeforeTool', 'AfterTool', 'Notification', 'PreCompress')

function Install-Gemini {
    Write-Host 'Installing AgentTray hooks for Gemini CLI...'
    Ensure-JsonFile -Path $GeminiSettings
    $settings = Read-JsonFile -Path $GeminiSettings

    if (-not $settings.hooks) {
        $settings | Add-Member -NotePropertyName 'hooks' -NotePropertyValue ([PSCustomObject]@{}) -Force
    }

    foreach ($event in $GeminiEvents) {
        $existing = @()
        if ($settings.hooks.PSObject.Properties[$event]) {
            $existing = Remove-AgentTrayFlat -EventEntries @($settings.hooks.$event)
        }

        $newEntry = [PSCustomObject]@{ type = 'command'; command = $HookCommand; tag = $HookTag }
        $existing += $newEntry

        if ($settings.hooks.PSObject.Properties[$event]) {
            $settings.hooks.$event = $existing
        } else {
            $settings.hooks | Add-Member -NotePropertyName $event -NotePropertyValue $existing -Force
        }
    }

    Write-JsonFile -Path $GeminiSettings -Data $settings
    Write-Host "  -> Updated $GeminiSettings"
}

function Uninstall-Gemini {
    Write-Host 'Removing AgentTray hooks from Gemini CLI...'
    if (-not (Test-Path $GeminiSettings)) {
        Write-Host '  -> No settings file found, nothing to do.'
        return
    }

    $settings = Read-JsonFile -Path $GeminiSettings
    if (-not $settings.hooks) { return }

    $propsToRemove = @()
    foreach ($prop in $settings.hooks.PSObject.Properties) {
        $cleaned = Remove-AgentTrayFlat -EventEntries @($prop.Value)
        if ($cleaned.Count -eq 0) {
            $propsToRemove += $prop.Name
        } else {
            $settings.hooks.($prop.Name) = $cleaned
        }
    }
    foreach ($name in $propsToRemove) {
        $settings.hooks.PSObject.Properties.Remove($name)
    }

    if (($settings.hooks.PSObject.Properties | Measure-Object).Count -eq 0) {
        $settings.PSObject.Properties.Remove('hooks')
    }

    Write-JsonFile -Path $GeminiSettings -Data $settings
    Write-Host "  -> Cleaned $GeminiSettings"
}

# ── Dispatch ──────────────────────────────────────────────────

function Run-For {
    param([string]$CliName)
    if ($Uninstall) {
        switch ($CliName) {
            'claude' { Uninstall-Claude }
            'codex'  { Uninstall-Codex }
            'gemini' { Uninstall-Gemini }
        }
    } else {
        switch ($CliName) {
            'claude' { Install-Claude }
            'codex'  { Install-Codex }
            'gemini' { Install-Gemini }
        }
    }
}

# Deploy hook scripts before installing
if (-not $Uninstall) {
    Deploy-HookScripts
}

switch ($Target) {
    'claude' { Run-For -CliName 'claude' }
    'codex'  { Run-For -CliName 'codex' }
    'gemini' { Run-For -CliName 'gemini' }
    'all' {
        Run-For -CliName 'claude'
        Run-For -CliName 'codex'
        Run-For -CliName 'gemini'
    }
}

Write-Host 'Done.'
