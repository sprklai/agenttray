#!/usr/bin/env bash
# wrap.sh <agent-name> <command> [args...]
# Wraps an AI agent command and writes real-time status to ~/.agent-monitor/
set -uo pipefail

if [ $# -lt 2 ]; then
  echo "Usage: wrap.sh <agent-name> <command> [args...]" >&2
  exit 1
fi

AGENT_NAME="$1"; shift
MONITOR_DIR="$HOME/.agent-monitor"
FILE="$MONITOR_DIR/$AGENT_NAME.status"

mkdir -p "$MONITOR_DIR"

# Detect terminal
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TERMINAL_JSON=$(bash "$SCRIPT_DIR/registry.sh" 2>/dev/null)
[ -z "$TERMINAL_JSON" ] && TERMINAL_JSON='{"kind":"unknown","focus_id":"'$$'","outer_id":"","label":"Terminal"}'

# Input detection patterns (ERE)
# Generic prompts
INPUT_PAT='\? $|\? .*\[|\[y/n\]|\[Y/n\]|\[yes/no\]|password:|Password:|passphrase:|Passphrase:|Enter to|Press .* to|Overwrite\?|Continue\?|Confirm\?|Proceed\?|Are you sure'
# Claude Code specific: permission prompts, tool approval
INPUT_PAT="$INPUT_PAT|Allow |Deny |approve|Yes, allow|No, deny|Do you want to|permission"
# Codex specific (future-proof)
INPUT_PAT="$INPUT_PAT|APPROVE|DENY|approve changes"
# User-supplied extra patterns
[ -n "${INPUT_EXTRA:-}" ] && INPUT_PAT="$INPUT_PAT|$INPUT_EXTRA"

# JSON-encode a string value (proper escaping of \, ", control chars)
_json_str() {
  local s="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$s" | jq -Rs .
  else
    # Bash fallback: escape backslash, double-quote, and control chars
    s="${s//\\/\\\\}"
    s="${s//\"/\\\"}"
    s="${s//$'\n'/\\n}"
    s="${s//$'\r'/\\r}"
    s="${s//$'\t'/\\t}"
    printf '"%s"' "$s"
  fi
}

# Atomic write helper
_write_status() {
  local status="$1" message="$2"
  message="${message:0:500}"
  local safe
  safe=$(_json_str "$message")
  printf '{"v":1,"status":"%s","message":%s,"terminal":%s}\n' \
    "$status" "$safe" "$TERMINAL_JSON" > "$FILE.tmp"
  mv -f "$FILE.tmp" "$FILE"
}

# Cleanup: remove status file so dead agents don't linger
_cleanup() {
  rm -f "$FILE" "$FILE.tmp"
  exit 130
}
trap _cleanup SIGTERM SIGINT SIGHUP

# Write starting status
_write_status "starting" ""

# Run the command, capturing stdout+stderr
"$@" 2>&1 | while IFS= read -r line; do
  if printf '%s' "$line" | grep -qE "$INPUT_PAT"; then
    _write_status "needs-input" "$line"
  else
    _write_status "working" "$line"
  fi
done

# Check exit code of the piped command
EXIT_CODE=${PIPESTATUS[0]}
if [ "$EXIT_CODE" -eq 0 ]; then
  # Clean exit — remove status file so agent doesn't linger
  rm -f "$FILE" "$FILE.tmp"
else
  _write_status "error" "Exit code $EXIT_CODE"
fi
