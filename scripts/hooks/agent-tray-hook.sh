#!/usr/bin/env bash
# agent-tray-hook.sh — Universal hook bridge for AgentTray
# Called by Claude Code, Codex CLI, and Gemini CLI hook systems.
# Reads hook event JSON from stdin, maps it to AgentTray status,
# and writes atomic status files to ~/.agent-monitor/.
set -uo pipefail

MONITOR_DIR="${HOME}/.agent-monitor"
mkdir -p "${MONITOR_DIR}"

# ── CLI Detection ──────────────────────────────────────────────

detect_cli() {
  if [ -n "${CLAUDE_SESSION_ID:-}" ]; then
    echo "claude-code"
  elif [ "${GEMINI_CLI:-}" = "1" ] || [ -n "${GEMINI_SESSION_ID:-}" ]; then
    echo "gemini"
  elif [ -n "${CODEX_SESSION_ID:-}" ]; then
    echo "codex"
  else
    # Fallback: check parent process name
    local parent=""
    if command -v ps >/dev/null 2>&1; then
      parent=$(ps -o comm= -p "${PPID}" 2>/dev/null || true)
    fi
    case "${parent}" in
      *claude*) echo "claude-code" ;;
      *codex*)  echo "codex" ;;
      *gemini*) echo "gemini" ;;
      *)        echo "unknown" ;;
    esac
  fi
}

CLI=$(detect_cli)

# ── Session ID ─────────────────────────────────────────────────

get_session_id() {
  case "${CLI}" in
    claude-code) echo "${CLAUDE_SESSION_ID:-$$}" ;;
    codex)       echo "${CODEX_SESSION_ID:-$$}" ;;
    gemini)      echo "${GEMINI_SESSION_ID:-$$}" ;;
    *)           echo "$$" ;;
  esac
}

SESSION_ID=$(get_session_id)
# Short session for filename (first 8 chars)
SESSION_SHORT="${SESSION_ID:0:8}"
STATUS_FILE="${MONITOR_DIR}/${CLI}-${SESSION_SHORT}.status"

# ── Terminal Info ──────────────────────────────────────────────

build_terminal_json() {
  local kind="unknown"
  local focus_id=""
  local outer_id=""
  local label="${TERM_PROGRAM:-Terminal}"
  local window_title=""

  local uname_s
  uname_s="$(uname -s 2>/dev/null || echo Unknown)"

  if [[ "$uname_s" == "Darwin" ]]; then
    # macOS: focuser expects kind="macos_app", focus_id=app name, outer_id=tty
    kind="macos_app"
    outer_id=$(tty 2>/dev/null | sed 's|/dev/||' || true)
    case "${TERM_PROGRAM:-}" in
      iTerm.app)       focus_id="iTerm2";   label="iTerm2" ;;
      Apple_Terminal)   focus_id="Terminal";  label="Terminal" ;;
      WezTerm)          focus_id="WezTerm";   label="WezTerm" ;;
      *)                focus_id="${TERM_PROGRAM:-}"; label="${TERM_PROGRAM:-Terminal}" ;;
    esac
  elif [[ "$uname_s" == MINGW* ]] || [[ "$uname_s" == MSYS* ]] || [[ "$uname_s" == CYGWIN* ]]; then
    # Windows via Git Bash / MSYS2 / Cygwin
    kind="windows_native"
    if [ -n "${PPID:-}" ]; then
      focus_id="${PPID}"
    fi
    if [ -n "${WT_SESSION:-}" ]; then
      label="Windows Terminal"
    elif [ -n "${ConEmuPID:-}" ]; then
      label="ConEmu"
    elif [ -n "${TERM_PROGRAM:-}" ]; then
      label="${TERM_PROGRAM}"
    else
      label="Git Bash"
    fi
  else
    # Linux/other: focuser expects kind="x11_generic", focus_id=X11 window ID (hex)
    # Scanner produces hex format (0x...) so we must match it for dedup
    if [ -n "${WINDOWID:-}" ]; then
      focus_id=$(printf '0x%x' "$WINDOWID")
    fi
    if [ -n "${TERM_PROGRAM:-}" ]; then
      kind="x11_generic"
    elif [ -n "${KITTY_PID:-}" ]; then
      kind="x11_generic"; label="Kitty"
    elif [ -n "${ALACRITTY_SOCKET:-}" ]; then
      kind="x11_generic"; label="Alacritty"
    elif [ -n "${WINDOWID:-}" ]; then
      kind="x11_generic"
    fi
  fi

  printf '{"kind":"%s","focus_id":"%s","outer_id":"%s","label":"%s","window_title":"%s"}' \
    "${kind}" "${focus_id}" "${outer_id}" "${label}" "${window_title}"
}

TERMINAL_JSON=$(build_terminal_json)

# ── JSON Helpers ───────────────────────────────────────────────

# Safely encode a string for JSON (handles special chars)
json_str() {
  local s="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$s" | jq -Rs .
  else
    s="${s//\\/\\\\}"
    s="${s//\"/\\\"}"
    s="${s//$'\n'/\\n}"
    s="${s//$'\r'/\\r}"
    s="${s//$'\t'/\\t}"
    printf '"%s"' "$s"
  fi
}

# Read a field from the input JSON (requires jq)
json_field() {
  local json="$1" field="$2"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$json" | jq -r ".${field} // empty" 2>/dev/null
  else
    # Crude fallback: grep for the field
    printf '%s' "$json" | grep -oP "\"${field}\"\s*:\s*\"?\K[^,\"}\]]*" 2>/dev/null | head -1
  fi
}

# ── Status File Writer ─────────────────────────────────────────

write_status() {
  local status="$1" message="$2" hook_event="$3" hook_matcher="${4:-}"

  # Truncate message
  message="${message:0:500}"
  local safe_msg
  safe_msg=$(json_str "$message")

  cat > "${STATUS_FILE}.tmp" <<EOJSON
{"v":1,"status":"${status}","message":${safe_msg},"source":"hook","cli":"${CLI}","session_id":"${SESSION_ID}","hook_event":"${hook_event}","hook_matcher":"${hook_matcher}","terminal":${TERMINAL_JSON}}
EOJSON
  mv -f "${STATUS_FILE}.tmp" "${STATUS_FILE}"
}

delete_status() {
  rm -f "${STATUS_FILE}" "${STATUS_FILE}.tmp"
}

# ── Event Mapping ──────────────────────────────────────────────

# Read stdin (hook systems pipe JSON)
INPUT=""
if [ ! -t 0 ]; then
  INPUT=$(cat)
fi

# Claude Code uses "hook_event_name"; other CLIs may use "event" or "type"
EVENT=$(json_field "$INPUT" "hook_event_name" 2>/dev/null || true)
[ -z "$EVENT" ] && EVENT=$(json_field "$INPUT" "event" 2>/dev/null || true)
[ -z "$EVENT" ] && EVENT=$(json_field "$INPUT" "type" 2>/dev/null || true)
[ -z "$EVENT" ] && EVENT=$(json_field "$INPUT" "hook_name" 2>/dev/null || true)

# Get matcher / subtype for filtering
MATCHER=""
# Claude Code: notification_type or tool_name
NOTIFICATION_TYPE=$(json_field "$INPUT" "notification_type" 2>/dev/null || true)
TOOL_NAME=$(json_field "$INPUT" "tool_name" 2>/dev/null || true)
[ -n "$NOTIFICATION_TYPE" ] && MATCHER="$NOTIFICATION_TYPE"
[ -n "$TOOL_NAME" ] && MATCHER="$TOOL_NAME"

# ── Claude Code Mapping ───────────────────────────────────────

map_claude_code() {
  case "$EVENT" in
    SessionStart)
      write_status "working" "Session started" "$EVENT"
      ;;
    Notification)
      case "$MATCHER" in
        permission_prompt)
          write_status "needs-input" "Waiting for permission" "$EVENT" "$MATCHER"
          ;;
        idle_prompt)
          write_status "needs-input" "Waiting for input" "$EVENT" "$MATCHER"
          ;;
        elicitation_dialog)
          write_status "needs-input" "MCP input requested" "$EVENT" "$MATCHER"
          ;;
        *)
          write_status "needs-input" "Notification: ${MATCHER}" "$EVENT" "$MATCHER"
          ;;
      esac
      ;;
    Stop)
      write_status "needs-input" "Waiting for input" "$EVENT"
      ;;
    StopFailure)
      write_status "error" "API error" "$EVENT"
      ;;
    UserPromptSubmit)
      write_status "working" "Processing prompt" "$EVENT"
      ;;
    PreToolUse)
      case "$MATCHER" in
        Bash)   write_status "working" "Running command..." "$EVENT" "$MATCHER" ;;
        Edit|Write) write_status "working" "Editing files..." "$EVENT" "$MATCHER" ;;
        Agent)  write_status "working" "Running subagent..." "$EVENT" "$MATCHER" ;;
        *)      write_status "working" "Using ${MATCHER}..." "$EVENT" "$MATCHER" ;;
      esac
      ;;
    PostToolUse)
      write_status "working" "Tool completed" "$EVENT" "$MATCHER"
      ;;
    SubagentStop)
      write_status "working" "Subagent completed" "$EVENT"
      ;;
    SessionEnd)
      delete_status
      ;;
    *)
      # Unknown event — ignore silently
      ;;
  esac
}

# ── Codex CLI Mapping ─────────────────────────────────────────

map_codex() {
  case "$EVENT" in
    SessionStart)
      write_status "working" "Session started" "$EVENT"
      ;;
    PreToolUse)
      write_status "working" "Running tool..." "$EVENT" "$MATCHER"
      ;;
    PostToolUse)
      write_status "working" "Tool completed" "$EVENT" "$MATCHER"
      ;;
    Stop)
      write_status "idle" "Finished responding" "$EVENT"
      ;;
    UserPromptSubmit)
      write_status "working" "Processing prompt" "$EVENT"
      ;;
    SessionEnd)
      delete_status
      ;;
    *)
      ;;
  esac
}

# ── Gemini CLI Mapping ────────────────────────────────────────

map_gemini() {
  case "$EVENT" in
    SessionStart)
      write_status "working" "Session started" "$EVENT"
      ;;
    SessionEnd)
      delete_status
      ;;
    BeforeAgent)
      write_status "working" "Processing..." "$EVENT"
      ;;
    AfterAgent)
      write_status "idle" "Finished responding" "$EVENT"
      ;;
    BeforeTool)
      local tool_label="${MATCHER:-tool}"
      write_status "working" "Running tool: ${tool_label}" "$EVENT" "$MATCHER"
      ;;
    AfterTool)
      local tool_label="${MATCHER:-tool}"
      write_status "working" "Tool completed: ${tool_label}" "$EVENT" "$MATCHER"
      ;;
    Notification)
      write_status "needs-input" "Waiting for input" "$EVENT" "$MATCHER"
      ;;
    PreCompress)
      write_status "working" "Compacting context" "$EVENT"
      ;;
    *)
      ;;
  esac
}

# ── Dispatch ──────────────────────────────────────────────────

case "$CLI" in
  claude-code) map_claude_code ;;
  codex)       map_codex ;;
  gemini)      map_gemini ;;
  *)
    # Unknown CLI — try to map generic events
    case "$EVENT" in
      SessionStart)     write_status "working" "Session started" "$EVENT" ;;
      SessionEnd)       delete_status ;;
      Stop)             write_status "idle" "Finished responding" "$EVENT" ;;
      Notification)     write_status "needs-input" "Notification" "$EVENT" "$MATCHER" ;;
      PreToolUse|BeforeTool) write_status "working" "Running tool..." "$EVENT" "$MATCHER" ;;
      *) ;;
    esac
    ;;
esac

# Hook scripts must exit 0 to not block the CLI
exit 0
