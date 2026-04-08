# Scripts

These scripts are how hook setup, custom CLI support, and local builds work today.

## Common Tasks

### Install Hooks for Supported CLIs

Unix:

```bash
./scripts/hooks/install-hooks.sh all
./scripts/hooks/install-hooks.sh claude
./scripts/hooks/install-hooks.sh codex
./scripts/hooks/install-hooks.sh gemini
./scripts/hooks/install-hooks.sh all --uninstall
```

Windows:

```powershell
.\scripts\hooks\install-hooks.ps1 -Target all
.\scripts\hooks\install-hooks.ps1 -Target claude
.\scripts\hooks\install-hooks.ps1 -Target codex
.\scripts\hooks\install-hooks.ps1 -Target gemini
.\scripts\hooks\install-hooks.ps1 -Target all -Uninstall
```

Notes:

- Unix hook installation requires `jq`
- Installers merge AgentTray entries into CLI settings instead of overwriting them
- AgentTray entries are tagged as `"agent-tray"` so uninstall removes only its own hooks

Settings files modified:

| CLI | File |
| --- | --- |
| Claude Code | `~/.claude/settings.json` |
| Codex CLI | `~/.codex/hooks.json` |
| Gemini CLI | `~/.gemini/settings.json` |

### Wrap Unsupported CLIs

Unix:

```bash
./scripts/wrap.sh my-agent -- claude chat
```

Windows:

```powershell
.\scripts\wrap.ps1 my-agent -- claude chat
```

This creates `~/.agent-monitor/my-agent.status` with atomic JSON updates that AgentTray can watch.

Example payload:

```json
{
  "v": 1,
  "status": "working",
  "message": "Processing input...",
  "terminal": {
    "kind": "x11_generic",
    "focus_id": "12345678",
    "outer_id": "",
    "label": "Terminal"
  }
}
```

### Build Locally

```bash
./scripts/build.sh --dev
./scripts/build.sh --release
./scripts/build.sh --release --bundle deb,appimage
```

PowerShell:

```powershell
.\scripts\build.ps1 -Dev
.\scripts\build.ps1 -Release
```

## Script Reference

### `scripts/hooks/agent-tray-hook.sh` and `scripts/hooks/agent-tray-hook.ps1`

Universal hook bridges for supported CLIs. They read hook event JSON, map it to AgentTray states, and write status files into `~/.agent-monitor/`.

### `scripts/registry.sh`

Terminal detector registry. It sources scripts from `scripts/detectors/` and returns the first matching terminal descriptor as JSON.

### `scripts/detectors/`

Add a terminal detector as `NN_name.sh`. Each detector should print JSON with `kind`, `focus_id`, and `label` when it matches, or print nothing otherwise.

Adding a new terminal type usually means:

1. Add a detector in `scripts/detectors/`
2. Add a Rust focuser in `src-tauri/src/focusers/`
3. Register the new focuser in `src-tauri/src/focusers/mod.rs`

## Maintainer Docs

- Release process: [../docs/maintainers/releasing.md](../docs/maintainers/releasing.md)
