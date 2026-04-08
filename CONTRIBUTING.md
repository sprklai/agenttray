# Contributing to AgentTray

Thank you for your interest in contributing to AgentTray! This guide will help you get started.

## Where to Help

Two areas have labeled issues ready for contributors:

### 1. Add CLI hook support

The hook bridge is a universal shell script (`scripts/hooks/agent-tray-hook.sh`) that Claude Code, Codex CLI, and Gemini CLI call on their events. Adding a new CLI takes ~30–50 lines:

1. Add detection logic in `detect_cli()` — typically 2–3 env var checks
2. Add an event→status mapping for that CLI's hook format (`working` / `needs-input` / `idle` / `error`)
3. Add an `install_<cli>()` function that merges hook entries into that CLI's settings file

See existing implementations for Claude Code and Codex as templates. Issues: [#7 Aider](https://github.com/sprklai/agenttray/issues/7), [#8 Amp](https://github.com/sprklai/agenttray/issues/8).

### 2. Add terminal focus support

The focus dispatcher lives in `src-tauri/src/focusers/` — one Rust impl per terminal kind. Each focuser receives a `FocusTarget` (terminal kind + session ID) and brings that window/pane to front.

To add a new terminal:
1. Create `src-tauri/src/focusers/<terminal>.rs` implementing the `Focuser` trait
2. Add detection in `src-tauri/src/focus.rs` using env vars (`$TERM_PROGRAM`, `$KITTY_PID`, etc.)
3. Wire it into the `dispatch()` match

See `focusers/kitty.rs` or `focusers/tmux.rs` as templates. Issue: [#12 Ghostty](https://github.com/sprklai/agenttray/issues/12).

All open `help wanted` and `good first issue` issues: [browse by label](https://github.com/sprklai/agenttray/issues?q=is%3Aopen+label%3A%22help+wanted%22%2C%22good+first+issue%22).

To claim an issue, comment `/assign` on it — you'll be automatically assigned without needing write access.

---

## Getting Started

1. **Fork** the repository on GitHub
2. **Clone** your fork locally:
   ```bash
   git clone https://github.com/<your-username>/agenttray.git
   cd agenttray
   ```
3. **Install prerequisites**:
   - Rust stable (via [rustup](https://rustup.rs/))
   - [Bun](https://bun.sh/)
   - Tauri CLI: `cargo install tauri-cli`
   - Linux system dependencies:
     ```bash
     sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
     ```

4. **Install and run**:
   ```bash
   bun install
   cargo tauri dev
   ```

## Branch Naming

Use descriptive branch names with one of these prefixes:

- `feature/` -- new features (e.g., `feature/macos-focus`)
- `fix/` -- bug fixes (e.g., `fix/tray-icon-flicker`)
- `docs/` -- documentation changes (e.g., `docs/hook-setup`)

## Code Style

AgentTray follows the conventions documented in [CLAUDE.md](CLAUDE.md). Key points:

- **Rust**: `snake_case` naming, `tracing` macros for logging, no `println!`
- **TypeScript/Svelte**: `camelCase` naming, Svelte 5 runes syntax
- **Frontend**: Tailwind CSS v4, Lucide icons
- **Imports**: std, then external crates, then internal modules (blank-line separated)
- **No dead code**: No commented-out code, unused imports, or placeholder stubs

## Testing Requirements

All PRs must pass these checks locally before submission:

```bash
# Rust checks
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --check

# Frontend checks
bun run check
```

## Commit Messages

- Use **imperative mood** (e.g., "Add macOS focus handler" not "Added macOS focus handler")
- Keep the subject line under 72 characters
- One logical change per commit -- do not bundle unrelated changes
- Reference related issues when applicable (e.g., "Fix #12: correct tray icon on dark theme")

Examples:
```
Add Gemini CLI hook detection
Fix popup not closing on focus loss
Update notify crate to v7 for macOS FSEvents
```

## Pull Request Process

### PR Checklist

Before submitting your PR, verify:

- [ ] Tests added or updated for all changed behavior
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml` passes
- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` passes
- [ ] `cargo fmt --manifest-path src-tauri/Cargo.toml --check` passes
- [ ] `bun run check` passes (if frontend changes)
- [ ] Documentation updated (if applicable)
- [ ] No breaking changes (or clearly documented in PR description)
- [ ] No secrets or credentials committed

### Review Process

1. Submit your PR with a clear description of the changes and motivation.
2. A maintainer will review your PR, typically within a few business days.
3. Address any requested changes by pushing additional commits (do not force-push during review).
4. Once approved, a maintainer will merge your PR.

### What to Expect

- PRs that add new features should include tests and documentation.
- PRs that fix bugs should include a test that reproduces the bug.
- Large architectural changes should be discussed in an issue first.
- Maintainers may suggest alternative approaches or request changes.

## Reporting Issues

- **Bugs**: [Open an issue](https://github.com/sprklai/agenttray/issues/new) with steps to reproduce, expected vs actual behavior, and your OS/platform.
- **Features**: [Open an issue](https://github.com/sprklai/agenttray/issues/new) describing the use case and proposed solution.

## License

By contributing to AgentTray, you agree that your contributions will be licensed under the [MIT License](LICENSE).
