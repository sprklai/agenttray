#!/usr/bin/env bash
# install-hooks.sh — Install/uninstall AgentTray hooks for AI CLI tools
# Usage: install-hooks.sh [claude|codex|gemini|all] [--uninstall]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HOOK_SCRIPT_SRC="${SCRIPT_DIR}/agent-tray-hook.sh"
HOOK_SCRIPT_PS1_SRC="${SCRIPT_DIR}/agent-tray-hook.ps1"

# Stable install location (not tied to source repo)
HOOK_INSTALL_DIR="${HOME}/.agent-monitor/hooks"

# ── Platform Detection ───────────────────────────────────────

PLATFORM="unix"
case "$(uname -s 2>/dev/null || echo Unknown)" in
  MINGW*|MSYS*|CYGWIN*) PLATFORM="windows" ;;
esac

# ── Args ──────────────────────────────────────────────────────

TARGET="${1:-all}"
UNINSTALL=false
for arg in "$@"; do
  [ "$arg" = "--uninstall" ] && UNINSTALL=true
done

# ── Prerequisites ─────────────────────────────────────────────

if ! command -v jq >/dev/null 2>&1; then
  echo "ERROR: jq is required. Install it with your package manager." >&2
  exit 1
fi

if [ ! -f "$HOOK_SCRIPT_SRC" ]; then
  echo "ERROR: Hook script not found: $HOOK_SCRIPT_SRC" >&2
  exit 1
fi

# ── Helpers ───────────────────────────────────────────────────

# Ensure a JSON file exists with at least {}
ensure_json_file() {
  local file="$1"
  local dir
  dir=$(dirname "$file")
  mkdir -p "$dir"
  if [ ! -f "$file" ]; then
    echo '{}' > "$file"
  fi
}

# Atomic write: write to PID-qualified .tmp then mv (avoids collision if
# two installer instances run concurrently)
atomic_write() {
  local file="$1" content="$2"
  printf '%s\n' "$content" > "${file}.tmp.$$"
  mv -f "${file}.tmp.$$" "$file"
}

# Deploy hook script(s) to ~/.agent-monitor/hooks/
deploy_hook_scripts() {
  mkdir -p "$HOOK_INSTALL_DIR"
  cp -f "$HOOK_SCRIPT_SRC" "${HOOK_INSTALL_DIR}/agent-tray-hook.sh"
  chmod +x "${HOOK_INSTALL_DIR}/agent-tray-hook.sh"
  echo "  -> Deployed hook script to ${HOOK_INSTALL_DIR}/agent-tray-hook.sh"

  # On Windows, also deploy the PowerShell version
  if [ "$PLATFORM" = "windows" ] && [ -f "$HOOK_SCRIPT_PS1_SRC" ]; then
    cp -f "$HOOK_SCRIPT_PS1_SRC" "${HOOK_INSTALL_DIR}/agent-tray-hook.ps1"
    echo "  -> Deployed PowerShell hook to ${HOOK_INSTALL_DIR}/agent-tray-hook.ps1"
  fi
}

# Resolve the hook command for settings.json
# On Windows (Git Bash), use bash + Windows-style path
# On Unix, use the deployed script directly
get_hook_cmd() {
  local installed="${HOOK_INSTALL_DIR}/agent-tray-hook.sh"
  if [ "$PLATFORM" = "windows" ]; then
    local win_path
    if command -v cygpath >/dev/null 2>&1; then
      win_path=$(cygpath -w "$installed")
    else
      # Fallback: convert /c/Users/... to C:\Users\...
      win_path=$(echo "$installed" | sed 's|^/\([a-zA-Z]\)/|\1:\\|;s|/|\\|g')
    fi
    echo "bash \"${win_path}\""
  else
    echo "$installed"
  fi
}

HOOK_CMD=""  # set after deploy

# ── Claude Code ───────────────────────────────────────────────

CLAUDE_SETTINGS="${HOME}/.claude/settings.json"
CLAUDE_HOOK_TAG="agent-tray"

install_claude() {
  echo "Installing AgentTray hooks for Claude Code..."
  ensure_json_file "$CLAUDE_SETTINGS"

  local current
  current=$(cat "$CLAUDE_SETTINGS")

  # Claude Code requires the matcher + hooks array format:
  # {"matcher": "", "hooks": [{"type": "command", "command": "...", "tag": "..."}]}
  local hook_json
  hook_json=$(cat <<EOJSON
{
  "hooks": {
    "SessionStart": [
      {"matcher": "", "hooks": [{"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}]}
    ],
    "SessionEnd": [
      {"matcher": "", "hooks": [{"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}]}
    ],
    "Notification": [
      {"matcher": "", "hooks": [{"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}]}
    ],
    "Stop": [
      {"matcher": "", "hooks": [{"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}]}
    ],
    "StopFailure": [
      {"matcher": "", "hooks": [{"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}]}
    ],
    "UserPromptSubmit": [
      {"matcher": "", "hooks": [{"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}]}
    ],
    "PreToolUse": [
      {"matcher": "", "hooks": [{"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}]}
    ],
    "PostToolUse": [
      {"matcher": "", "hooks": [{"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}]}
    ],
    "SubagentStop": [
      {"matcher": "", "hooks": [{"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}]}
    ]
  }
}
EOJSON
)

  # Merge: handle both new matcher+hooks format and legacy flat entries
  local merged
  merged=$(printf '%s' "$current" | jq --argjson new "$hook_json" '
    .hooks //= {} |
    reduce ($new.hooks | to_entries[]) as $entry (
      .;
      .hooks[$entry.key] //= [] |
      # Remove existing agent-tray entries (handles both formats)
      .hooks[$entry.key] = [
        .hooks[$entry.key][] |
        if .hooks then
          # New format: filter agent-tray from nested hooks array
          .hooks = [.hooks[] | select(.tag != "agent-tray")] |
          select(.hooks | length > 0)
        else
          # Legacy flat format: filter by tag directly
          select(.tag != "agent-tray")
        end
      ] |
      # Append new matcher group
      .hooks[$entry.key] += $entry.value
    )
  ')

  atomic_write "$CLAUDE_SETTINGS" "$merged"
  echo "  -> Updated $CLAUDE_SETTINGS"
}

uninstall_claude() {
  echo "Removing AgentTray hooks from Claude Code..."
  if [ ! -f "$CLAUDE_SETTINGS" ]; then
    echo "  -> No settings file found, nothing to do."
    return
  fi

  local current
  current=$(cat "$CLAUDE_SETTINGS")

  local cleaned
  cleaned=$(printf '%s' "$current" | jq '
    if .hooks then
      .hooks |= with_entries(
        .value = [
          .value[] |
          if .hooks then
            # New format: filter agent-tray from nested hooks array
            .hooks = [.hooks[] | select(.tag != "agent-tray")] |
            select(.hooks | length > 0)
          else
            # Legacy flat format: filter by tag directly
            select(.tag != "agent-tray")
          end
        ] |
        select(.value | length > 0)
      ) |
      if (.hooks | length) == 0 then del(.hooks) else . end
    else . end
  ')

  atomic_write "$CLAUDE_SETTINGS" "$cleaned"
  echo "  -> Cleaned $CLAUDE_SETTINGS"
}

# ── Codex CLI ─────────────────────────────────────────────────

CODEX_SETTINGS="${HOME}/.codex/hooks.json"

install_codex() {
  echo "Installing AgentTray hooks for Codex CLI..."
  ensure_json_file "$CODEX_SETTINGS"

  local current
  current=$(cat "$CODEX_SETTINGS")

  local hook_json
  hook_json=$(cat <<EOJSON
{
  "hooks": {
    "SessionStart": [
      {"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "PreToolUse": [
      {"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "PostToolUse": [
      {"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "Stop": [
      {"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "UserPromptSubmit": [
      {"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}
    ]
  }
}
EOJSON
)

  local merged
  merged=$(printf '%s' "$current" | jq --argjson new "$hook_json" '
    .hooks //= {} |
    reduce ($new.hooks | to_entries[]) as $entry (
      .;
      .hooks[$entry.key] //= [] |
      .hooks[$entry.key] = [.hooks[$entry.key][] | select(.tag != "agent-tray")] |
      .hooks[$entry.key] += $entry.value
    )
  ')

  atomic_write "$CODEX_SETTINGS" "$merged"
  echo "  -> Updated $CODEX_SETTINGS"
}

uninstall_codex() {
  echo "Removing AgentTray hooks from Codex CLI..."
  if [ ! -f "$CODEX_SETTINGS" ]; then
    echo "  -> No settings file found, nothing to do."
    return
  fi

  local current
  current=$(cat "$CODEX_SETTINGS")

  local cleaned
  cleaned=$(printf '%s' "$current" | jq '
    if .hooks then
      .hooks |= with_entries(
        .value = [.value[] | select(.tag != "agent-tray")] |
        select(.value | length > 0)
      ) |
      if (.hooks | length) == 0 then del(.hooks) else . end
    else . end
  ')

  atomic_write "$CODEX_SETTINGS" "$cleaned"
  echo "  -> Cleaned $CODEX_SETTINGS"
}

# ── Gemini CLI ────────────────────────────────────────────────

GEMINI_SETTINGS="${HOME}/.gemini/settings.json"

install_gemini() {
  echo "Installing AgentTray hooks for Gemini CLI..."
  ensure_json_file "$GEMINI_SETTINGS"

  local current
  current=$(cat "$GEMINI_SETTINGS")

  local hook_json
  hook_json=$(cat <<EOJSON
{
  "hooks": {
    "SessionStart": [
      {"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "SessionEnd": [
      {"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "BeforeAgent": [
      {"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "AfterAgent": [
      {"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "BeforeTool": [
      {"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "AfterTool": [
      {"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "Notification": [
      {"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "PreCompress": [
      {"type": "command", "command": "${HOOK_CMD}", "tag": "${CLAUDE_HOOK_TAG}"}
    ]
  }
}
EOJSON
)

  local merged
  merged=$(printf '%s' "$current" | jq --argjson new "$hook_json" '
    .hooks //= {} |
    reduce ($new.hooks | to_entries[]) as $entry (
      .;
      .hooks[$entry.key] //= [] |
      .hooks[$entry.key] = [.hooks[$entry.key][] | select(.tag != "agent-tray")] |
      .hooks[$entry.key] += $entry.value
    )
  ')

  atomic_write "$GEMINI_SETTINGS" "$merged"
  echo "  -> Updated $GEMINI_SETTINGS"
}

uninstall_gemini() {
  echo "Removing AgentTray hooks from Gemini CLI..."
  if [ ! -f "$GEMINI_SETTINGS" ]; then
    echo "  -> No settings file found, nothing to do."
    return
  fi

  local current
  current=$(cat "$GEMINI_SETTINGS")

  local cleaned
  cleaned=$(printf '%s' "$current" | jq '
    if .hooks then
      .hooks |= with_entries(
        .value = [.value[] | select(.tag != "agent-tray")] |
        select(.value | length > 0)
      ) |
      if (.hooks | length) == 0 then del(.hooks) else . end
    else . end
  ')

  atomic_write "$GEMINI_SETTINGS" "$cleaned"
  echo "  -> Cleaned $GEMINI_SETTINGS"
}

# ── Dispatch ──────────────────────────────────────────────────

run_for() {
  local cli="$1"
  if $UNINSTALL; then
    case "$cli" in
      claude) uninstall_claude ;;
      codex)  uninstall_codex ;;
      gemini) uninstall_gemini ;;
    esac
  else
    case "$cli" in
      claude) install_claude ;;
      codex)  install_codex ;;
      gemini) install_gemini ;;
    esac
  fi
}

# Deploy hook scripts before installing (skip for uninstall)
if ! $UNINSTALL; then
  deploy_hook_scripts
  HOOK_CMD=$(get_hook_cmd)
fi

case "$TARGET" in
  claude) run_for claude ;;
  codex)  run_for codex ;;
  gemini) run_for gemini ;;
  all)
    run_for claude
    run_for codex
    run_for gemini
    ;;
  *)
    echo "Usage: $(basename "$0") [claude|codex|gemini|all] [--uninstall]" >&2
    exit 1
    ;;
esac

echo "Done."
