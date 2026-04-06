# Contributing to AgentTray

Thank you for your interest in contributing to AgentTray! This guide will help you get started.

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
