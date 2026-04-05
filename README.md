# AgentTray

A system tray app that monitors AI coding agents in real-time and displays their status at a glance. Built with Tauri 2, SvelteKit, and Rust.

## Features

- **Live status monitoring** — colored tray icon reflects the most urgent agent state
- **Popup dashboard** — click the tray icon to see all agents with status, message, and focus button
- **Terminal focus** — jump directly to the terminal running a specific agent
- **Process scanning** — automatically detects running Claude CLI instances via `/proc`
- **File-based status** — agents report status via simple JSON files in `~/.agent-monitor/`
- **Global hotkey** — `Ctrl+Shift+A` toggles the popup from anywhere
- **Lightweight** — no background services, no database, just file watchers and `/proc` reads

## Status States

| State        | Color  | Meaning                        |
|--------------|--------|--------------------------------|
| needs-input  | Yellow | Agent is waiting for user input |
| error        | Red    | Agent exited with non-zero code |
| working      | Blue   | Agent is actively processing    |
| starting     | Cyan   | Agent just launched             |
| idle         | Green  | Agent running, minimal CPU      |
| offline      | Gray   | No status file found            |

## How It Works

```
wrap.sh writes ~/.agent-monitor/<name>.status (JSON)
  → watcher.rs detects file change (inotify)
  → emits "agents-updated" Tauri event
  → Svelte popup re-renders agent list
  → tray icon color updates to match aggregate state
```

Agents are wrapped with `scripts/wrap.sh`, which detects the terminal type, monitors stdout for input prompts, and writes atomic status updates. The Rust backend watches the status directory and also scans `/proc` for live Claude CLI processes to merge both sources.

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

## Build Commands

```bash
bun install                  # Install frontend dependencies
bun run dev                  # Start SvelteKit dev server only
bun run build                # Build frontend to ./build/
bun run check                # TypeScript/Svelte type checking
cargo tauri dev              # Full app in dev mode
cargo tauri build            # Production build
```

## Wrapping an Agent

```bash
./scripts/wrap.sh my-agent -- claude chat
```

Creates `~/.agent-monitor/my-agent.status` with real-time JSON status updates. See `scripts/README.md` for all script documentation.

## Project Structure

```
├── scripts/
│   ├── build.sh              # Build automation
│   ├── wrap.sh               # Agent command wrapper
│   ├── registry.sh           # Terminal detector registry
│   └── detectors/            # Terminal detection scripts
├── src/
│   ├── routes/+page.svelte   # Popup UI
│   ├── lib/components/       # Svelte components (AgentRow, StatusDot, etc.)
│   ├── lib/types.ts          # TypeScript interfaces
│   └── lib/utils.ts          # Utility functions
├── src-tauri/
│   ├── src/main.rs           # App entry, tray setup, watcher spawn
│   ├── src/watcher.rs        # File watcher + /proc scanner
│   ├── src/tray.rs           # Tray icon + popup window management
│   ├── src/focus.rs          # Terminal focus command router
│   ├── src/focusers/         # Platform-specific focus handlers
│   ├── icons/                # Tray state icons (22x22 PNGs)
│   └── Cargo.toml            # Rust dependencies
├── CLAUDE.md                 # Claude Code project instructions
├── plan.md                   # Detailed architecture spec
└── package.json              # Frontend dependencies
```

## Tech Stack

- **Backend:** Rust + Tauri 2.x (tray-icon, shell, process, global-shortcut plugins)
- **Frontend:** SvelteKit + Svelte 5 (runes) + Tailwind CSS v4
- **File watching:** notify crate v6 (inotify on Linux)
- **Icons:** Lucide Svelte

## Releasing

Releases are built automatically by [GitHub Actions](.github/workflows/release.yml) for all platforms:

| Platform | Formats |
|----------|---------|
| Linux    | `.deb`, `.rpm`, `.AppImage` |
| Windows  | `.msi`, `.exe` (NSIS) |
| macOS    | `.dmg` (arm64 + x86_64) |

See `scripts/README.md` for release script usage and required secrets.

## License

MIT

