#!/usr/bin/env bash
# AgentTray -- Build Script
# Usage: ./scripts/build.sh [OPTIONS]
#
# Options:
#   --dev                 Start dev mode (Vite + Tauri dev server)
#   --release             Build in release mode (default: debug)
#   --bundle <FORMATS>    Comma-separated bundle formats (e.g., deb,appimage)
#   --help                Show this help message

set -euo pipefail

# ── Configuration ──────────────────────────────────────────────────────
WORKSPACE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Extract dev port from vite.config.ts (fallback to 5173)
DEV_PORT=$(grep -oP 'port:\s*\K[0-9]+' "${WORKSPACE_ROOT}/vite.config.ts" 2>/dev/null || echo "5173")

# ── Defaults ───────────────────────────────────────────────────────────
DEV_MODE=false
PROFILE="debug"
BUNDLE_FORMATS=""

# ── Colors ─────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
err()   { echo -e "${RED}[ERROR]${NC} $*" >&2; }
step()  { echo -e "${CYAN}[STEP]${NC}  $*"; }

# ── Functions ──────────────────────────────────────────────────────────

show_help() {
    head -9 "$0" | tail -8 | sed 's/^# //' | sed 's/^#//'
}

check_deps() {
    if ! command -v bun &> /dev/null; then
        err "bun is not installed. Install it from https://bun.sh"
        exit 1
    fi

    if ! cargo tauri --version &> /dev/null; then
        err "cargo-tauri CLI is not installed."
        echo "  Install it with: cargo install tauri-cli"
        exit 1
    fi
}

install_frontend_deps() {
    if [ ! -d "${WORKSPACE_ROOT}/node_modules" ]; then
        info "Installing frontend dependencies..."
        (cd "${WORKSPACE_ROOT}" && bun install)
    fi
}

run_dev() {
    info "Starting AgentTray dev mode..."
    check_deps
    install_frontend_deps

    # Kill any existing process on the dev port
    local existing_pid
    existing_pid=$(lsof -ti :"${DEV_PORT}" 2>/dev/null || true)
    if [ -n "$existing_pid" ]; then
        warn "Port ${DEV_PORT} is in use (PID $existing_pid), killing it..."
        kill "$existing_pid" 2>/dev/null || true
        sleep 1
    fi

    step "Launching cargo tauri dev..."
    cd "${WORKSPACE_ROOT}"
    cargo tauri dev --no-watch
}

run_build() {
    info "Building AgentTray..."
    check_deps
    install_frontend_deps

    # Build frontend first
    step "Building frontend assets..."
    (cd "${WORKSPACE_ROOT}" && bun run build)
    ok "Frontend built"

    # Assemble cargo tauri build args
    local tauri_args=()

    if [ "$PROFILE" = "debug" ]; then
        tauri_args+=("--debug")
    fi

    if [ -n "$BUNDLE_FORMATS" ]; then
        IFS=',' read -ra FORMATS <<< "$BUNDLE_FORMATS"
        for fmt in "${FORMATS[@]}"; do
            tauri_args+=("--bundles" "$fmt")
        done
    fi

    step "Running: cargo tauri build ${tauri_args[*]:-}"
    cd "${WORKSPACE_ROOT}"
    cargo tauri build "${tauri_args[@]}"

    if [ $? -eq 0 ]; then
        ok "Build complete!"
        echo ""
        info "Bundle outputs:"
        local bundle_dir="${WORKSPACE_ROOT}/src-tauri/target/release/bundle"
        if [ "$PROFILE" = "debug" ]; then
            bundle_dir="${WORKSPACE_ROOT}/src-tauri/target/debug/bundle"
        fi
        if [ -d "$bundle_dir" ]; then
            find "$bundle_dir" -type f \( -name "*.deb" -o -name "*.AppImage" -o -name "*.dmg" -o -name "*.msi" -o -name "*.exe" -o -name "*.rpm" \) 2>/dev/null | sort | while read -r f; do
                local_path="${f#"$WORKSPACE_ROOT"/}"
                size=$(du -h "$f" | awk '{print $1}')
                echo "  ${local_path} (${size})"
            done
        fi
    else
        err "Build failed!"
        exit 1
    fi
}

# ── Parse Arguments ────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dev)
            DEV_MODE=true
            shift
            ;;
        --release)
            PROFILE="release"
            shift
            ;;
        --bundle)
            BUNDLE_FORMATS="$2"
            shift 2
            ;;
        --help|-h)
            show_help
            exit 0
            ;;
        *)
            err "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# ── Main ───────────────────────────────────────────────────────────────

if [ "$DEV_MODE" = true ]; then
    run_dev
else
    echo ""
    echo "========================================"
    echo "  AgentTray Build"
    echo "  Profile: $PROFILE"
    if [ -n "$BUNDLE_FORMATS" ]; then
    echo "  Bundles: $BUNDLE_FORMATS"
    fi
    echo "========================================"
    echo ""
    run_build
fi
