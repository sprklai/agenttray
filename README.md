# AgentTray

### Know the moment your AI needs you.

Real-time tray notifications for Claude, Codex, and Gemini — surfaces agent pauses that need your input, instantly.

[![GitHub Release](https://img.shields.io/github/v/release/sprklai/agenttray?style=flat-square&label=latest)](https://github.com/sprklai/agenttray/releases/latest)
[![License](https://img.shields.io/github/license/sprklai/agenttray?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-blue?style=flat-square)](#download)
[![Built with Tauri](https://img.shields.io/badge/built_with-Tauri_2-24C8DB?style=flat-square&logo=tauri&logoColor=white)](https://tauri.app)
[![Stars](https://img.shields.io/github/stars/sprklai/agenttray?style=flat-square)](https://github.com/sprklai/agenttray/stargazers)

<img src="assets/AdobeAgentTray.gif" alt="AgentTray — real-time AI agent monitor" width="800" />

**Live agent status. Input alerts. Zero tab-switching.**

## Download

[![Linux](https://img.shields.io/badge/Linux-.deb%20%7C%20.AppImage%20%7C%20.rpm-orange?style=for-the-badge&logo=linux&logoColor=white)](https://github.com/sprklai/agenttray/releases/latest)
[![macOS](https://img.shields.io/badge/macOS-.dmg%20(arm64%20%2B%20x64)-black?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/sprklai/agenttray/releases/latest)
[![Windows](https://img.shields.io/badge/Windows-.msi%20%7C%20.exe-0078D4?style=for-the-badge&logo=windows&logoColor=white)](https://github.com/sprklai/agenttray/releases/latest)

> Pick the right format for your distro/arch from the [releases page](https://github.com/sprklai/agenttray/releases/latest).

---

A system tray app that monitors AI coding agents in real-time and displays their status at a glance. Built with Tauri 2, SvelteKit, and Rust.

Supports **Claude Code**, **Codex CLI**, and **Gemini CLI** out of the box — via native hooks or process scanning.

## Features

- **Live status monitoring** — colored tray icon reflects the most urgent agent state
- **Audio + desktop alerts** — plays a system beep and sends a native desktop notification the moment an agent needs input
- **Popup dashboard** — click the tray icon to see all agents with status, message, and focus button
- **Terminal focus** — jump directly to the terminal running a specific agent
- **Native hook integration** — installs lightweight hooks into Claude Code, Codex CLI, and Gemini CLI for instant status updates
- **Process scanning** — automatically detects running CLI instances (Linux, macOS, Windows) as a fallback
- **Source-aware dedup** — merges hook-reported and process-scanned agents without duplicates
- **File-based status** — agents report status via simple JSON files in `~/.agent-monitor/`
- **Global hotkey** — `Ctrl+Shift+A` toggles the popup from anywhere
- **Lightweight** — no background services, no database, just file watchers and hooks

## Status States

| State        | Color  | Meaning                        |
|--------------|--------|--------------------------------|
| needs-input  | Yellow | Agent is waiting for user input — triggers beep + desktop notification |
| error        | Red    | Agent exited with non-zero code |
| working      | Blue   | Agent is actively processing    |
| starting     | Cyan   | Agent just launched             |
| idle         | Green  | Agent running, minimal CPU      |
| offline      | Gray   | No status file found            |

## How It Works

AgentTray uses two complementary strategies to track agents:

### 1. Hooks (recommended)

```
CLI hook fires → agent-tray-hook.sh maps event to status
  → writes ~/.agent-monitor/<cli>-<session>.status (JSON)
  → watcher.rs detects file change (inotify)
  → emits "agents-updated" Tauri event
  → Svelte popup re-renders agent list
  → tray icon color updates to match aggregate state
  → if needs-input transition: plays system beep + sends desktop notification
```

Hooks are installed into each CLI's settings file and fire on events like session start, tool use, notifications, and stop. The hook script auto-detects which CLI is calling it and maps events to the appropriate status.

### 2. Process scanning (fallback)

```
scanner/ detects running CLI processes (Linux /proc, macOS, Windows)
  → merges with hook-reported agents (dedup by session)
  → emits "agents-updated" Tauri event
```

Process scanning works without any setup but provides less granular status (just running/not running). Hook-sourced agents take priority when both sources report the same session.

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Bun](https://bun.sh/)
- Tauri CLI: `cargo install tauri-cli`
- Linux system dependencies (Ubuntu/Debian):
  ```bash
  sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
  ```

## Getting Started

```bash
# Clone
git clone https://github.com/sprklai/agenttray.git
cd agenttray

# Install frontend dependencies
bun install

# Run in dev mode
cargo tauri dev
```

## Installing Hooks

The recommended way to connect AgentTray to your AI CLI tools:

```bash
# Install hooks for all supported CLIs
./scripts/hooks/install-hooks.sh all

# Or install for a specific CLI
./scripts/hooks/install-hooks.sh claude
./scripts/hooks/install-hooks.sh codex
./scripts/hooks/install-hooks.sh gemini

# Uninstall
./scripts/hooks/install-hooks.sh all --uninstall
```

Requires `jq` to be installed.

### What the installer does

The installer **merges** hook entries into each CLI's settings file — it does not overwrite existing settings. All entries are tagged with `"agent-tray"` for clean identification and removal.

| CLI          | Settings file modified                |
|--------------|---------------------------------------|
| Claude Code  | `~/.claude/settings.json`             |
| Codex CLI    | `~/.codex/hooks.json`                 |
| Gemini CLI   | `~/.gemini/settings.json`             |

> **Note:** Claude Code hooks must live in `~/.claude/settings.json` — there is no alternative location. The installer safely merges using `jq` and the `--uninstall` flag cleanly removes only AgentTray entries, leaving all other settings intact.

### Alternative: wrap.sh

For agents that don't support hooks, or for custom commands:

```bash
./scripts/wrap.sh my-agent -- claude chat
```

Creates `~/.agent-monitor/my-agent.status` with real-time JSON status updates. See `scripts/README.md` for all script documentation.

## Build Commands

```bash
bun install                  # Install frontend dependencies
bun run dev                  # Start SvelteKit dev server only
bun run build                # Build frontend to ./build/
bun run check                # TypeScript/Svelte type checking
cargo tauri dev              # Full app in dev mode
cargo tauri build            # Production build
```

## Project Structure

```
├── scripts/
│   ├── build.sh              # Build automation
│   ├── wrap.sh               # Agent command wrapper
│   ├── registry.sh           # Terminal detector registry
│   ├── detectors/            # Terminal detection scripts
│   └── hooks/
│       ├── install-hooks.sh  # Hook installer/uninstaller
│       └── agent-tray-hook.sh # Universal hook bridge script
├── src/
│   ├── routes/+page.svelte   # Popup UI
│   ├── lib/components/       # Svelte components (AgentRow, StatusDot, etc.)
│   ├── lib/types.ts          # TypeScript interfaces
│   └── lib/utils.ts          # Utility functions
├── src-tauri/
│   ├── src/main.rs           # App entry, tray setup, watcher spawn
│   ├── src/watcher.rs        # File watcher, event loop, orphan cleanup
│   ├── src/scanner/          # Cross-platform process scanner (Linux, macOS, Windows)
│   ├── src/heuristics.rs     # CPU/state heuristics for status classification
│   ├── src/notifier.rs       # Desktop notification dispatch
│   ├── src/tray.rs           # Tray icon + popup window management
│   ├── src/focus.rs          # Terminal focus command router
│   ├── src/focusers/         # Platform-specific focus handlers
│   ├── icons/                # App and tray icons
│   └── Cargo.toml            # Rust dependencies
├── CLAUDE.md                 # Claude Code project instructions
└── package.json              # Frontend dependencies
```

## Tech Stack

- **Backend:** Rust + Tauri 2.x (tray-icon, shell, process, global-shortcut, notification, window-state plugins)
- **Frontend:** SvelteKit + Svelte 5 (runes) + Tailwind CSS v4
- **File watching:** notify crate v8
- **Icons:** Lucide Svelte

## Releasing

Releases are built automatically by [GitHub Actions](.github/workflows/release.yml) for all platforms:

| Platform | Formats |
|----------|---------|
| Linux    | `.deb`, `.rpm`, `.AppImage` |
| Windows  | `.msi`, `.exe` (NSIS) |
| macOS    | `.dmg` (arm64 + x86_64) |

See `scripts/README.md` for release script usage and required secrets.

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=sprklai/agenttray&type=Date)](https://star-history.com/#sprklai/agenttray)

## License

MIT

