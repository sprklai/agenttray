#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

DRY_RUN=false
PUSH=false
VERSION=""

usage() {
    echo "Usage: $0 [--dry-run] [--push] <version>"
    echo ""
    echo "Sync version across all project files, commit, tag, and optionally push."
    echo ""
    echo "Arguments:"
    echo "  version      Semver version (e.g., 1.2.0)"
    echo ""
    echo "Options:"
    echo "  --dry-run    Show changes without applying them"
    echo "  --push       Push commit and tag to origin (triggers release workflow)"
    echo "  -h, --help   Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 0.2.0              # Version bump, commit, tag (no push)"
    echo "  $0 --push 0.2.0      # Full release: bump, commit, tag, push"
    echo "  $0 --dry-run 0.2.0   # Preview only"
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --push)
            PUSH=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        -*)
            echo "Error: Unknown option $1"
            usage
            exit 1
            ;;
        *)
            VERSION="$1"
            shift
            ;;
    esac
done

if [[ -z "$VERSION" ]]; then
    echo "Error: Version argument required"
    usage
    exit 1
fi

# Validate semver format
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
    echo "Error: Invalid semver format: $VERSION"
    echo "Expected format: MAJOR.MINOR.PATCH (e.g., 1.2.0)"
    exit 1
fi

# --- Pre-flight checks ---
BRANCH=$(git -C "$ROOT_DIR" rev-parse --abbrev-ref HEAD)
if [[ "$BRANCH" != "main" ]]; then
    echo "Error: Must be on 'main' branch (currently on '$BRANCH')"
    exit 1
fi

if [[ "$DRY_RUN" == false ]]; then
    if ! git -C "$ROOT_DIR" diff --quiet || ! git -C "$ROOT_DIR" diff --cached --quiet; then
        echo "Error: Working tree has uncommitted changes. Commit or stash them first."
        exit 1
    fi
fi

TAG="app-v$VERSION"
if git -C "$ROOT_DIR" rev-parse "$TAG" >/dev/null 2>&1; then
    echo "Error: Tag '$TAG' already exists"
    exit 1
fi

CARGO_TOML="$ROOT_DIR/src-tauri/Cargo.toml"
PACKAGE_JSON="$ROOT_DIR/package.json"
TAURI_CONF="$ROOT_DIR/src-tauri/tauri.conf.json"

echo "=== AgentTray Release: v$VERSION ==="
echo ""

# --- Read current versions ---
CARGO_OLD=$(grep -P '^version' "$CARGO_TOML" | head -1 | grep -oP '"\K[^"]+')
PKG_OLD=$(grep -oP '(?<="version": ")[^"]+' "$PACKAGE_JSON")
TAURI_OLD=$(grep -oP '(?<="version": ")[^"]+' "$TAURI_CONF" | head -1)

echo "[1/3] src-tauri/Cargo.toml: $CARGO_OLD -> $VERSION"
echo "[2/3] package.json: $PKG_OLD -> $VERSION"
echo "[3/3] src-tauri/tauri.conf.json: $TAURI_OLD -> $VERSION"
echo ""

if [[ "$DRY_RUN" == true ]]; then
    echo "[dry-run] No files modified. No tag created."
    echo "[dry-run] Would create git tag: $TAG"
    if [[ "$PUSH" == true ]]; then
        echo "[dry-run] Would push: git push origin main && git push origin $TAG"
    fi
    echo ""

    LAST_TAG=$(git -C "$ROOT_DIR" describe --tags --abbrev=0 2>/dev/null || echo "")
    echo "=== Changelog ==="
    if [[ -n "$LAST_TAG" ]]; then
        echo "Changes since $LAST_TAG:"
        git -C "$ROOT_DIR" log --oneline "$LAST_TAG"..HEAD
    else
        echo "All commits:"
        git -C "$ROOT_DIR" log --oneline -20
    fi
    echo ""
    echo "Done (dry-run)."
    exit 0
fi

# --- Update versions ---
sed -i "0,/^version[[:space:]]*= \"$CARGO_OLD\"/s//version     = \"$VERSION\"/" "$CARGO_TOML"
sed -i "s/\"version\": \"$PKG_OLD\"/\"version\": \"$VERSION\"/" "$PACKAGE_JSON"
sed -i "s/\"version\": \"$TAURI_OLD\"/\"version\": \"$VERSION\"/" "$TAURI_CONF"

# --- Commit and tag ---
git -C "$ROOT_DIR" add "$CARGO_TOML" "$PACKAGE_JSON" "$TAURI_CONF"
git -C "$ROOT_DIR" commit -m "release: v$VERSION"
git -C "$ROOT_DIR" tag "$TAG"
echo "Committed and tagged: $TAG"

# --- Changelog ---
echo ""
LAST_TAG=$(git -C "$ROOT_DIR" describe --tags --abbrev=0 --exclude="$TAG" 2>/dev/null || echo "")
echo "=== Changelog ==="
if [[ -n "$LAST_TAG" ]]; then
    echo "Changes since $LAST_TAG:"
    git -C "$ROOT_DIR" log --oneline "$LAST_TAG".."$TAG"
else
    echo "All commits:"
    git -C "$ROOT_DIR" log --oneline -20
fi

# --- Push ---
echo ""
if [[ "$PUSH" == true ]]; then
    echo "Pushing to origin..."
    git -C "$ROOT_DIR" push origin main
    git -C "$ROOT_DIR" push origin "$TAG"
    echo ""
    echo "Release triggered. Monitor at:"
    echo "  https://github.com/sprklai/agenttray/actions"
else
    echo "To trigger the release workflow:"
    echo "  git push origin main && git push origin $TAG"
fi

echo ""
echo "Done."
