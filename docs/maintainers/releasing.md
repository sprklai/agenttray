# Releasing AgentTray

This document is for maintainers. It covers version bumps, tagging, CI release builds, and required signing secrets.

## Release Script

`scripts/release.sh` syncs version numbers across `src-tauri/Cargo.toml`, `package.json`, and `src-tauri/tauri.conf.json`, then creates the git tag that triggers the release build.

Examples:

```bash
./scripts/release.sh 0.2.0
./scripts/release.sh --push 0.2.0
./scripts/release.sh --dry-run 0.2.0
```

## Pre-flight Checks

- be on `main`
- keep the working tree clean
- make sure the tag does not already exist
- verify that release notes and docs match the shipped behavior

## CI Release Artifacts

The GitHub Actions workflow builds:

- Linux: `.deb`, `.rpm`, `.AppImage`
- Windows: `.msi`, `.exe`
- macOS: `.dmg` for arm64 and x86_64

The workflow lives in [../../.github/workflows/release.yml](../../.github/workflows/release.yml).

## Required GitHub Secrets

macOS signing currently requires these repository secrets:

| Secret | Purpose |
| --- | --- |
| `APPLE_CERTIFICATE` | Base64-encoded `.p12` Developer ID certificate |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the `.p12` file |
| `KEYCHAIN_PASSWORD` | CI keychain password |
| `APPLE_SIGNING_IDENTITY` | Signing identity string |
| `APPLE_ID` | Apple ID email for notarization |
| `APPLE_PASSWORD` | App-specific password for notarization |

`GITHUB_TOKEN` is provided automatically by GitHub Actions.

## Manual Workflow Dispatch

You can also trigger release builds from the [Actions workflow page](https://github.com/sprklai/agenttray/actions/workflows/release.yml) with:

- selected platforms
- an optional version override
- publish on or off
