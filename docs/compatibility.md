# AgentTray Compatibility

AgentTray detects sessions in two ways:

- hooks: better state changes and better input detection
- process scanning: rougher fallback with less setup

## CLI Support

| CLI | Hook integration | Scan detection | Prompt-level alerts | Notes |
| --- | --- | --- | --- | --- |
| Claude Code | Yes | Yes | Yes | This is the path used most during development |
| Codex CLI | Yes | Yes | Yes | Hook and scan results are deduped by session |
| Gemini CLI | Yes | Yes | Yes | Hook mapping is supported for session, tool, and notification events |
| Other CLIs | Manual | No built-in strategy | Manual/file-based only | Use `scripts/wrap.sh` or add a strategy |

## Platform Status

| Platform | App builds | Scan detection | Hook installer | Focus support | Current confidence |
| --- | --- | --- | --- | --- | --- |
| Linux | `.deb`, `.rpm`, `.AppImage` | Yes | Shell installer | Yes | Used most during development |
| macOS | `.dmg` for arm64 and x64 | Yes | Shell installer | Yes | Supported in code, less tested than Linux |
| Windows | `.msi` and `.exe` | Yes | PowerShell installer | Yes | Supported in code, less tested than Linux |

## Focus Support Notes

Dedicated focus routes exist for:

- generic X11 terminals
- macOS app terminals
- native Windows terminals
- Kitty, tmux, GNU screen, WezTerm, Zellij, and Neovim
- VS Code and JetBrains terminals

If AgentTray cannot capture enough focus metadata for your terminal, monitoring still works but the focus button may be unavailable or unreliable.

## Setup Notes

- `scripts/hooks/install-hooks.sh` requires `jq`
- hook installers merge into CLI settings instead of overwriting them
- status files are stored locally in `~/.agent-monitor/`
- source checkout is still the easiest full setup path because the hook installers live in this repo

## Known Limitations

- Linux gets the most use during development
- process scanning is less precise than hooks
- terminal focus quality depends on terminal or multiplexer metadata and OS window tooling
