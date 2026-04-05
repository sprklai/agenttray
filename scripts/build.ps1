# AgentTray -- Build Script (PowerShell)
# Usage: .\scripts\build.ps1 [OPTIONS]
#
# Options:
#   -Dev                  Start dev mode (Vite + Tauri dev server)
#   -Release              Build in release mode (default: debug)
#   -Bundle <FORMATS>     Comma-separated bundle formats (e.g., msi,nsis)
#   -Help                 Show this help message

param(
    [switch]$Dev,
    [switch]$Release,
    [string]$Bundle = '',
    [switch]$Help
)

$ErrorActionPreference = 'Stop'

# ── Configuration ──────────────────────────────────────────────────────
$WorkspaceRoot = Split-Path -Parent $PSScriptRoot

# Extract dev port from vite.config.ts (fallback to 5173)
$DevPort = 5173
$ViteConfig = Join-Path $WorkspaceRoot 'vite.config.ts'
if (Test-Path $ViteConfig) {
    $match = Select-String -Path $ViteConfig -Pattern 'port:\s*(\d+)' | Select-Object -First 1
    if ($match) { $DevPort = [int]$match.Matches[0].Groups[1].Value }
}

$Profile = if ($Release) { 'release' } else { 'debug' }

# ── Colors ─────────────────────────────────────────────────────────────
function Write-Info  { param($msg) Write-Host "[INFO]  $msg" -ForegroundColor Blue }
function Write-Ok    { param($msg) Write-Host "[OK]    $msg" -ForegroundColor Green }
function Write-Warn  { param($msg) Write-Host "[WARN]  $msg" -ForegroundColor Yellow }
function Write-Err   { param($msg) Write-Host "[ERROR] $msg" -ForegroundColor Red }
function Write-Step  { param($msg) Write-Host "[STEP]  $msg" -ForegroundColor Cyan }

# ── Functions ──────────────────────────────────────────────────────────

function Show-Help {
    Get-Content $PSCommandPath | Select-Object -Skip 1 -First 8 | ForEach-Object { $_ -replace '^# ?', '' }
}

function Test-Dependencies {
    if (-not (Get-Command bun -ErrorAction SilentlyContinue)) {
        Write-Err "bun is not installed. Install it from https://bun.sh"
        exit 1
    }
    try { cargo tauri --version 2>$null | Out-Null } catch {
        Write-Err "cargo-tauri CLI is not installed."
        Write-Host "  Install it with: cargo install tauri-cli"
        exit 1
    }
}

function Install-FrontendDeps {
    $nodeModules = Join-Path $WorkspaceRoot 'node_modules'
    if (-not (Test-Path $nodeModules)) {
        Write-Info "Installing frontend dependencies..."
        Push-Location $WorkspaceRoot
        bun install
        Pop-Location
    }
}

function Start-Dev {
    Write-Info "Starting AgentTray dev mode..."
    Test-Dependencies
    Install-FrontendDeps

    # Kill any existing process on the dev port
    $existingPid = (Get-NetTCPConnection -LocalPort $DevPort -ErrorAction SilentlyContinue).OwningProcess | Select-Object -First 1
    if ($existingPid) {
        Write-Warn "Port $DevPort is in use (PID $existingPid), killing it..."
        Stop-Process -Id $existingPid -Force -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 1
    }

    Write-Step "Launching cargo tauri dev..."
    Push-Location $WorkspaceRoot
    cargo tauri dev --no-watch
    Pop-Location
}

function Start-Build {
    Write-Info "Building AgentTray..."
    Test-Dependencies
    Install-FrontendDeps

    # Build frontend first
    Write-Step "Building frontend assets..."
    Push-Location $WorkspaceRoot
    bun run build
    Pop-Location
    Write-Ok "Frontend built"

    # Assemble cargo tauri build args
    $tauriArgs = @()
    if ($Profile -eq 'debug') { $tauriArgs += '--debug' }

    if ($Bundle) {
        foreach ($fmt in ($Bundle -split ',')) {
            $tauriArgs += '--bundles'
            $tauriArgs += $fmt.Trim()
        }
    }

    Write-Step "Running: cargo tauri build $($tauriArgs -join ' ')"
    Push-Location $WorkspaceRoot
    cargo tauri build @tauriArgs
    Pop-Location

    if ($LASTEXITCODE -eq 0) {
        Write-Ok "Build complete!"
        Write-Host ""
        Write-Info "Bundle outputs:"
        $bundleDir = Join-Path $WorkspaceRoot "src-tauri\target\$Profile\bundle"
        if (Test-Path $bundleDir) {
            Get-ChildItem -Path $bundleDir -Recurse -Include '*.msi','*.exe','*.nsis' | ForEach-Object {
                $relativePath = $_.FullName.Replace("$WorkspaceRoot\", '')
                $size = '{0:N1} MB' -f ($_.Length / 1MB)
                Write-Host "  $relativePath ($size)"
            }
        }
    } else {
        Write-Err "Build failed!"
        exit 1
    }
}

# ── Main ───────────────────────────────────────────────────────────────

if ($Help) {
    Show-Help
    exit 0
}

if ($Dev) {
    Start-Dev
} else {
    Write-Host ""
    Write-Host "========================================"
    Write-Host "  AgentTray Build"
    Write-Host "  Profile: $Profile"
    if ($Bundle) { Write-Host "  Bundles: $Bundle" }
    Write-Host "========================================"
    Write-Host ""
    Start-Build
}
