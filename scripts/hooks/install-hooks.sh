#!/usr/bin/env bash
# install-hooks.sh — Install/uninstall AgentTray hooks for AI CLI tools
# Usage: install-hooks.sh [claude|codex|gemini|all] [--uninstall]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HOOK_SCRIPT="${SCRIPT_DIR}/agent-tray-hook.sh"

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

if [ ! -x "$HOOK_SCRIPT" ]; then
  echo "ERROR: Hook script not found or not executable: $HOOK_SCRIPT" >&2
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

# Atomic write: write to .tmp then mv
atomic_write() {
  local file="$1" content="$2"
  printf '%s\n' "$content" > "${file}.tmp"
  mv -f "${file}.tmp" "$file"
}

# ── Claude Code ───────────────────────────────────────────────

CLAUDE_SETTINGS="${HOME}/.claude/settings.json"
CLAUDE_HOOK_TAG="agent-tray"

install_claude() {
  echo "Installing AgentTray hooks for Claude Code..."
  ensure_json_file "$CLAUDE_SETTINGS"

  local current
  current=$(cat "$CLAUDE_SETTINGS")

  # Build hook entries for all relevant events
  local hook_json
  hook_json=$(cat <<EOJSON
{
  "hooks": {
    "SessionStart": [
      {"type": "command", "command": "${HOOK_SCRIPT}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "Notification": [
      {"type": "command", "command": "${HOOK_SCRIPT}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "Stop": [
      {"type": "command", "command": "${HOOK_SCRIPT}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "PreToolUse": [
      {"type": "command", "command": "${HOOK_SCRIPT}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "SubagentStop": [
      {"type": "command", "command": "${HOOK_SCRIPT}", "tag": "${CLAUDE_HOOK_TAG}"}
    ]
  }
}
EOJSON
)

  # Merge: for each event, append our hook entries (avoiding duplicates)
  local merged
  merged=$(printf '%s' "$current" | jq --argjson new "$hook_json" '
    # Ensure .hooks exists
    .hooks //= {} |
    # For each event in new hooks, merge entries
    reduce ($new.hooks | to_entries[]) as $entry (
      .;
      .hooks[$entry.key] //= [] |
      # Remove any existing agent-tray hooks first
      .hooks[$entry.key] = [.hooks[$entry.key][] | select(.tag != "agent-tray")] |
      # Append new entries
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
        .value = [.value[] | select(.tag != "agent-tray")] |
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
      {"type": "command", "command": "${HOOK_SCRIPT}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "PreToolUse": [
      {"type": "command", "command": "${HOOK_SCRIPT}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "PostToolUse": [
      {"type": "command", "command": "${HOOK_SCRIPT}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "Stop": [
      {"type": "command", "command": "${HOOK_SCRIPT}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "UserPromptSubmit": [
      {"type": "command", "command": "${HOOK_SCRIPT}", "tag": "${CLAUDE_HOOK_TAG}"}
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
      {"type": "command", "command": "${HOOK_SCRIPT}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "SessionEnd": [
      {"type": "command", "command": "${HOOK_SCRIPT}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "BeforeAgent": [
      {"type": "command", "command": "${HOOK_SCRIPT}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "AfterAgent": [
      {"type": "command", "command": "${HOOK_SCRIPT}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "BeforeTool": [
      {"type": "command", "command": "${HOOK_SCRIPT}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "AfterTool": [
      {"type": "command", "command": "${HOOK_SCRIPT}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "Notification": [
      {"type": "command", "command": "${HOOK_SCRIPT}", "tag": "${CLAUDE_HOOK_TAG}"}
    ],
    "PreCompress": [
      {"type": "command", "command": "${HOOK_SCRIPT}", "tag": "${CLAUDE_HOOK_TAG}"}
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
