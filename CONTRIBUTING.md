# Contributing to AgentTray

AgentTray is still early. The best contributions are the ones that make the app work better in real use, not the ones that add the most code.

## Fast Local Setup

```bash
git clone https://github.com/<your-username>/agenttray.git
cd agenttray
bun install
cargo tauri dev
```

Prerequisites:

- Rust stable via [rustup](https://rustup.rs/)
- [Bun](https://bun.sh/)
- Tauri CLI: `cargo install tauri-cli`
- Ubuntu/Debian packages:
  ```bash
  sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
  ```

## Good Places to Help

### Add CLI Hook Support

The hook bridge lives in `scripts/hooks/agent-tray-hook.sh` and `scripts/hooks/install-hooks.sh`.

Typical work:

1. Add CLI detection in `detect_cli()`
2. Map hook events to `working`, `needs-input`, `idle`, or `error`
3. Add installer logic that merges AgentTray hooks into that CLI's settings

Good starting issues: [#7 Aider](https://github.com/sprklai/agenttray/issues/7), [#8 Amp](https://github.com/sprklai/agenttray/issues/8)

### Add Terminal Focus Support

Terminal focusers live in `src-tauri/src/focusers/`.

Typical work:

1. Add `src-tauri/src/focusers/<terminal>.rs`
2. Detect that terminal in the focus pipeline
3. Wire the new focuser into the dispatcher

Templates: `kitty.rs`, `tmux.rs`, `wezterm.rs`

### Validate Platforms and Onboarding

This helps a lot right now.

- Validate the current macOS or Windows path
- Tighten setup docs where the first-run experience is unclear
- Improve screenshots, demos, and compatibility notes

## Architecture Pointers

- `scripts/hooks/` installs hooks and translates CLI events into local status files
- `scripts/wrap.sh` and `scripts/wrap.ps1` are the fallback path for unsupported CLIs
- `src-tauri/src/scanner/strategies/` detects Claude Code, Codex CLI, and Gemini processes
- `src-tauri/src/focusers/` contains terminal and IDE focus implementations
- `src-tauri/src/watcher.rs` watches `~/.agent-monitor/` and emits UI updates
- `src/routes/+page.svelte` renders the popup list

## Quality Bar

Run the checks that match your change before opening a PR:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --check
bun run check
```

Expectations:

- add or update tests for behavior changes
- update docs when user-facing behavior changes
- keep PRs scoped to one logical change
- do not force-push during review

## Claiming Work

Browse the open [`help wanted`](https://github.com/sprklai/agenttray/issues?q=is%3Aopen+label%3A%22help+wanted%22) and [`good first issue`](https://github.com/sprklai/agenttray/issues?q=is%3Aopen+label%3A%22good+first+issue%22) labels.

To claim an issue, comment `/assign`.

For larger changes, open an issue first so the direction is clear before implementation.

## Commit and PR Guidance

- Use imperative commit subjects
- Keep the subject line under 72 characters
- Do not mix unrelated changes in one commit
- Include motivation, behavior change, and test notes in the PR description
- Call out breaking changes explicitly

Examples:

```text
Add Gemini CLI hook detection
Fix popup not closing on focus loss
Update docs for Windows hook installation
```

## License

By contributing to AgentTray, you agree that your contributions will be licensed under the [MIT License](LICENSE).
