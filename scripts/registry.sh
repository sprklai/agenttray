#!/usr/bin/env bash
# Sources every detectors/*.sh in lex order. Returns first non-empty JSON match.
# Each detector runs in a clean subshell — cannot affect siblings.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DETECTORS_DIR="$SCRIPT_DIR/detectors"

for detector in "$DETECTORS_DIR"/*.sh; do
  [ -f "$detector" ] || continue
  result=$(bash "$detector" 2>/dev/null)
  if [ -n "$result" ]; then
    printf '%s' "$result"
    exit 0
  fi
done

# Fallback (should never reach here given 99_unknown.sh)
printf '{"kind":"unknown","focus_id":"%d","outer_id":"","label":"Terminal"}\n' "$$"
