# Scripts

## build.sh

Build automation for dev and release modes.

```bash
./scripts/build.sh --dev                           # Dev mode
./scripts/build.sh --release                       # Release build
./scripts/build.sh --release --bundle deb,appimage # With specific bundles
```

## wrap.sh

Wraps any command-line agent to report real-time status via JSON files.

```bash
./scripts/wrap.sh my-agent -- claude chat
```

Creates `~/.agent-monitor/my-agent.status` with atomic JSON updates:

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

## release.sh

Syncs version across `src-tauri/Cargo.toml`, `package.json`, and `src-tauri/tauri.conf.json`, then commits and creates a git tag to trigger the GitHub Actions release workflow.

```bash
./scripts/release.sh 0.2.0              # Bump, commit, tag (manual push)
./scripts/release.sh --push 0.2.0      # Full release: bump, commit, tag, push
./scripts/release.sh --dry-run 0.2.0   # Preview changes without applying
```

### Pre-flight checks

- Must be on `main` branch
- Working tree must be clean (no uncommitted changes)
- Tag must not already exist

### Required GitHub Secrets

macOS signing requires these repository secrets (shared with the NSR Tech org):

| Secret | Purpose |
|--------|---------|
| `APPLE_CERTIFICATE` | Base64-encoded .p12 Developer ID certificate |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the .p12 file |
| `KEYCHAIN_PASSWORD` | CI keychain password (any value) |
| `APPLE_SIGNING_IDENTITY` | Signing identity string |
| `APPLE_ID` | Apple ID email for notarization |
| `APPLE_PASSWORD` | App-specific password for notarization |

`GITHUB_TOKEN` is provided automatically by GitHub Actions.

### Manual Dispatch

You can also trigger builds from the [Actions tab](https://github.com/sprklai/agenttray/actions/workflows/release.yml) with inputs for platform selection, version override, and publish toggle.

## hooks/

### install-hooks.sh

Installs/uninstalls AgentTray hook entries into AI CLI settings files. Uses `jq` to safely merge entries without overwriting existing settings.

```bash
./scripts/hooks/install-hooks.sh all              # Install for all CLIs
./scripts/hooks/install-hooks.sh claude            # Claude Code only
./scripts/hooks/install-hooks.sh codex             # Codex CLI only
./scripts/hooks/install-hooks.sh gemini            # Gemini CLI only
./scripts/hooks/install-hooks.sh all --uninstall   # Remove all hooks
```

All hook entries are tagged with `"agent-tray"` — the installer removes existing tagged entries before adding new ones (safe to re-run). The `--uninstall` flag removes only AgentTray entries.

**Settings files modified:**

| CLI          | File                          |
|--------------|-------------------------------|
| Claude Code  | `~/.claude/settings.json`     |
| Codex CLI    | `~/.codex/hooks.json`         |
| Gemini CLI   | `~/.gemini/settings.json`     |

### agent-tray-hook.sh

Universal hook bridge called by all supported CLIs. Reads hook event JSON from stdin, auto-detects which CLI is calling it, maps events to AgentTray status, and writes atomic status files to `~/.agent-monitor/`.

**Supported events by CLI:**

| Claude Code       | Codex CLI         | Gemini CLI        |
|-------------------|-------------------|-------------------|
| SessionStart      | SessionStart      | SessionStart      |
| Notification      | PreToolUse        | SessionEnd        |
| Stop              | PostToolUse       | BeforeAgent       |
| PreToolUse        | Stop              | AfterAgent        |
| SubagentStop      | UserPromptSubmit  | BeforeTool/AfterTool |
|                   |                   | Notification      |
|                   |                   | PreCompress       |

## registry.sh

Terminal detector registry. Sources detector scripts from `detectors/` and returns the first matching terminal info as JSON.

## detectors/

Add terminal detection scripts here as `NN_name.sh`. Each script should print JSON with `kind`, `focus_id`, and `label` fields if it detects a matching terminal, or print nothing otherwise.

### Adding a New Terminal Type

1. **Detector** — add `scripts/detectors/NN_name.sh`
2. **Focuser** — add `src-tauri/src/focusers/name.rs` implementing the focus logic
3. **Register** — add one line to `src-tauri/src/focusers/mod.rs` dispatch match
