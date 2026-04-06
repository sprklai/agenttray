# Release

Create a versioned release: commit, tag, push, and GitHub release.

## Arguments

$ARGUMENTS — the version number (e.g. `0.2.0`). Required.

## Instructions

Perform the following steps for version `$ARGUMENTS`:

1. **Bump version** in all version files:
   - `src-tauri/Cargo.toml` — `version = "$ARGUMENTS"`
   - `src-tauri/tauri.conf.json` — `"version": "$ARGUMENTS"`
   - `package.json` — `"version": "$ARGUMENTS"`
   - Run `cargo check --manifest-path src-tauri/Cargo.toml` to update `Cargo.lock`

2. **Review changes** — run `git status` and `git diff --stat` to see all staged + unstaged changes. Show a summary to the user.

3. **Commit** — stage all changed files and create a commit:
   - Message format: `release: v$ARGUMENTS` followed by a blank line and a concise summary of what changed (read `git diff` to determine this)
   - Include `Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>`

4. **Tag** — create an annotated tag: `git tag -a v$ARGUMENTS -m "v$ARGUMENTS"`

5. **Push** — push commits and tag: `git push && git push origin v$ARGUMENTS`

6. **GitHub Release** — create a release via `gh release create v$ARGUMENTS` with:
   - Title: `v$ARGUMENTS`
   - Body: auto-generated changelog grouped by commit type (feat/fix/docs/chore). Include a "Full Changelog" compare link to the previous tag.

7. **Verify** — run `git status` and print the release URL.

If any step fails, stop and report the error. Do not proceed to the next step.
