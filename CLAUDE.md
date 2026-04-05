# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

AgentTray — a cross-platform system tray app (Tauri 2.x + SvelteKit + Rust) that monitors AI coding agents and displays their real-time status via a colored dot in the system tray. Part of NSRTech.

## Tech Stack

- **Backend:** Rust + Tauri 2.x (tray-icon, shell, process plugins)
- **Frontend:** SvelteKit + Svelte 5 (runes) + Tailwind CSS v4
- **Package manager:** bun (not npm)
- **Icons:** @lucide/svelte
- **File watcher:** notify crate v6 (inotify on Linux)
- **Build:** `bun run build` for frontend, `cargo tauri dev` for full app

## Commands

- `bun install` — install frontend dependencies
- `bun run build` — build frontend to `build/`
- `bun run dev` — start SvelteKit dev server
- `cargo tauri dev` — run full Tauri app in dev mode
- `cargo tauri build` — production build
- `cargo test --manifest-path src-tauri/Cargo.toml` — run Rust tests
- `bun run check` — TypeScript/Svelte type checking

## Architecture

- Status files: `~/.agent-monitor/<name>.status` (JSON v1 format)
- Shell scripts: `scripts/wrap.sh` wraps agent commands, `scripts/registry.sh` detects terminal
- Rust watcher: `src-tauri/src/watcher.rs` watches status dir, emits `agents-updated` events
- Tray: `src-tauri/src/tray.rs` manages icon color and popup window
- Focus: `src-tauri/src/focusers/` dispatches terminal focus by kind
- Frontend: Svelte popup renders agent list from Tauri events

## Status

MVP implementation complete. Core loop functional on Linux.
