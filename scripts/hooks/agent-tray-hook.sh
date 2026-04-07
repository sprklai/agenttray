#!/usr/bin/env bash
# agent-tray-hook.sh — Universal hook bridge for AgentTray
# Called by Claude Code, Codex CLI, and Gemini CLI hook systems.
# Reads hook event JSON from stdin, maps it to AgentTray status,
# and writes atomic status files to ~/.agent-monitor/.
set -uo pipefail

MONITOR_DIR="${HOME}/.agent-monitor"
mkdir -p "${MONITOR_DIR}"

# ── Read stdin FIRST (hook systems pipe JSON) ─────────────────
# Must happen before CLI/session detection since those need the JSON.

INPUT=""
if [ ! -t 0 ]; then
  INPUT=$(cat)
fi

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

# Read a field from the input JSON
json_field() {
  local json="$1" field="$2"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$json" | jq -r ".${field} // empty" 2>/dev/null
  else
    # Crude fallback: grep for the field
    printf '%s' "$json" | grep -oP "\"${field}\"\s*:\s*\"?\K[^,\"}\]]*" 2>/dev/null | head -1
  fi
}

# ── CLI Detection ──────────────────────────────────────────────

detect_cli() {
  # Check CLI-specific env vars first (most specific indicators).
  # IMPORTANT: session_id in JSON is a common field for ALL CLIs,
  # so it must NOT be used as a Claude Code indicator.
  if [ "${GEMINI_CLI:-}" = "1" ] || [ -n "${GEMINI_SESSION_ID:-}" ]; then
    echo "gemini"
  elif [ -n "${CODEX_SESSION_ID:-}" ]; then
    echo "codex"
  elif [ -n "${CLAUDE_SESSION_ID:-}" ]; then
    echo "claude-code"
  else
    # Check JSON: hook_event_name is Claude Code-specific
    local hook_event_name
    hook_event_name=$(json_field "$INPUT" "hook_event_name" 2>/dev/null || true)
    if [ -n "$hook_event_name" ]; then
      echo "claude-code"
      return
    fi
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
# Derive a stable session identifier shared by the main session AND its
# subagents.  Priority: JSON session_id > CLI env var > process-tree walk.
# The walk finds the top-level CLI ancestor process; its PID is the same
# for hooks fired by the main session and any of its subagents.
# IS_SUBAGENT is set to "true" when the walk finds more than one CLI
# ancestor, meaning this hook was fired from a nested subagent process.

SESSION_ID=""
IS_SUBAGENT="false"

# 1. JSON session_id (most reliable when the CLI provides it)
_json_sid=$(json_field "$INPUT" "session_id" 2>/dev/null || true)
if [ -n "$_json_sid" ]; then
  SESSION_ID="$_json_sid"
fi

# 2. CLI-specific env vars
if [ -z "$SESSION_ID" ]; then
  case "${CLI}" in
    claude-code) SESSION_ID="${CLAUDE_SESSION_ID:-}" ;;
    codex)       SESSION_ID="${CODEX_SESSION_ID:-}" ;;
    gemini)      SESSION_ID="${GEMINI_SESSION_ID:-}" ;;
  esac
fi

# 3. Walk the process tree to find the top-level CLI ancestor.
if [ -z "$SESSION_ID" ]; then
  _walk_pid="${PPID}"
  _root_cli_pid=""
  _cli_depth=0
  for _i in 1 2 3 4 5 6 7 8 9 10; do
    [ -z "$_walk_pid" ] || [ "$_walk_pid" = "1" ] || [ "$_walk_pid" = "0" ] && break
    _cmd_line=$(ps -o command= -p "$_walk_pid" 2>/dev/null || true)
    [ -z "$_cmd_line" ] && break
    _exe_base=$(basename "$(echo "$_cmd_line" | awk '{print $1}')" 2>/dev/null || true)
    case "$_exe_base" in
      claude|claude-code|codex|gemini)
        _root_cli_pid="$_walk_pid"
        _cli_depth=$((_cli_depth + 1))
        ;;
      node|bun|deno|python|python3)
        # Check if running a CLI script (e.g. "node /path/to/claude/cli.js")
        if echo "$_cmd_line" | grep -qi 'claude\|codex\|gemini'; then
          _root_cli_pid="$_walk_pid"
          _cli_depth=$((_cli_depth + 1))
        fi
        ;;
    esac
    _walk_pid=$(ps -o ppid= -p "$_walk_pid" 2>/dev/null | tr -d ' ' || true)
  done
  if [ -n "$_root_cli_pid" ]; then
    SESSION_ID="$_root_cli_pid"
  else
    SESSION_ID="$$"
  fi
  [ "$_cli_depth" -gt 1 ] && IS_SUBAGENT="true"
fi
# Short session for filename (first 8 chars)
SESSION_SHORT="${SESSION_ID:0:8}"
STATUS_FILE="${MONITOR_DIR}/${CLI}-${SESSION_SHORT}.status"

# Working directory for display name (may be absent in some CLI versions)
CWD=$(json_field "$INPUT" "cwd" 2>/dev/null || echo "")

# ── Terminal Info ──────────────────────────────────────────────

build_terminal_json() {
  local kind="unknown"
  local focus_id=""
  local outer_id=""
  local label="${TERM_PROGRAM:-Terminal}"
  local window_title=""

  local uname_s
  uname_s="$(uname -s 2>/dev/null || echo Unknown)"

  # ── 1. Cross-platform multiplexers (highest priority) ───────
  # These wrap around the real terminal; detect them first so the
  # focuser can switch to the correct pane/session.

  if [ -n "${TMUX:-}" ]; then
    kind="tmux"
    # Capture current pane target: session:window.pane
    focus_id=$(tmux display-message -p '#{session_name}:#{window_index}.#{pane_index}' 2>/dev/null || echo "")
    label="tmux"
  elif [ -n "${STY:-}" ]; then
    kind="screen"
    focus_id="${STY}"
    outer_id="${WINDOW:-0}"
    label="GNU Screen"
  elif [ -n "${ZELLIJ_SESSION_NAME:-}" ]; then
    kind="zellij"
    focus_id="${ZELLIJ_SESSION_NAME}"
    label="Zellij"
  fi

  # ── 2. Neovim :terminal ─────────────────────────────────────
  if [ "$kind" = "unknown" ] && [ -n "${NVIM:-}" ]; then
    kind="neovim"
    focus_id="${NVIM}"   # socket path
    label="Neovim"
  fi

  # ── 3. IDE terminals (cross-platform) ───────────────────────
  if [ "$kind" = "unknown" ]; then
    case "${TERM_PROGRAM:-}" in
      vscode)
        kind="vscode"
        label="VS Code"
        ;;
    esac
    if [ "$kind" = "unknown" ] && [[ "${TERMINAL_EMULATOR:-}" == *JetBrains* ]]; then
      kind="jetbrains"
      label="JetBrains"
    fi
  fi

  # ── 4. Kitty (has its own remote-control API) ───────────────
  if [ "$kind" = "unknown" ] && [ -n "${KITTY_WINDOW_ID:-}" ]; then
    kind="kitty"
    focus_id="${KITTY_WINDOW_ID}"
    label="Kitty"
  fi

  # ── 5. Platform-specific terminals ──────────────────────────
  if [ "$kind" = "unknown" ]; then
    if [[ "$uname_s" == "Darwin" ]]; then
      kind="macos_app"
      # tty may return "not a tty" in hook subprocesses; handle gracefully
      local tty_raw
      tty_raw=$(tty 2>/dev/null || true)
      if [[ "$tty_raw" == "not a tty" ]] || [[ -z "$tty_raw" ]]; then
        outer_id=""
      else
        outer_id=$(echo "$tty_raw" | sed 's|/dev/||')
      fi
      case "${TERM_PROGRAM:-}" in
        iTerm.app)       focus_id="iTerm2";     label="iTerm2" ;;
        Apple_Terminal)   focus_id="Terminal";    label="Terminal" ;;
        WezTerm)          focus_id="WezTerm";     label="WezTerm" ;;
        ghostty)          focus_id="Ghostty";     label="Ghostty" ;;
        Hyper)            focus_id="Hyper";        label="Hyper" ;;
        Tabby|Terminus)   focus_id="Tabby";        label="Tabby" ;;
        WarpTerminal)     focus_id="Warp";         label="Warp" ;;
        *)                focus_id="${TERM_PROGRAM:-}"; label="${TERM_PROGRAM:-Terminal}" ;;
      esac
    elif [[ "$uname_s" == MINGW* ]] || [[ "$uname_s" == MSYS* ]] || [[ "$uname_s" == CYGWIN* ]]; then
      kind="windows_native"
      if [ -n "${PPID:-}" ]; then
        focus_id="${PPID}"
      fi
      if [ -n "${WT_SESSION:-}" ]; then
        label="Windows Terminal"
      elif [ -n "${ConEmuPID:-}" ]; then
        label="ConEmu"
      elif [ -n "${CMDER_ROOT:-}" ]; then
        label="Cmder"
      elif [ -n "${TERM_PROGRAM:-}" ]; then
        label="${TERM_PROGRAM}"
      else
        label="Git Bash"
      fi
    else
      # Linux / other Unix
      if [ -n "${WINDOWID:-}" ]; then
        focus_id=$(printf '0x%x' "$WINDOWID")
      fi
      if [ -n "${GHOSTTY_RESOURCES_DIR:-}" ]; then
        kind="x11_generic"; label="Ghostty"
      elif [ -n "${ALACRITTY_WINDOW_ID:-}" ] || [ -n "${ALACRITTY_SOCKET:-}" ]; then
        kind="x11_generic"; label="Alacritty"
      elif [ -n "${KONSOLE_VERSION:-}" ]; then
        kind="x11_generic"; label="Konsole"
      elif [ -n "${TERMINATOR_UUID:-}" ]; then
        kind="x11_generic"; label="Terminator"
      elif [ -n "${TILIX_ID:-}" ]; then
        kind="x11_generic"; label="Tilix"
      elif [ -n "${TERM_PROGRAM:-}" ]; then
        kind="x11_generic"
        case "${TERM_PROGRAM:-}" in
          WarpTerminal) label="Warp" ;;
          Hyper)        label="Hyper" ;;
          Tabby|Terminus) label="Tabby" ;;
          *)            label="${TERM_PROGRAM}" ;;
        esac
      elif [ -n "${KITTY_PID:-}" ]; then
        # Fallback: kitty without KITTY_WINDOW_ID (older versions)
        kind="x11_generic"; label="Kitty"
      elif [ -n "${WINDOWID:-}" ]; then
        kind="x11_generic"
      fi
    fi
  fi

  # ── Fallback: find X11 window via xdotool when WINDOWID unset ─
  # Mirrors the Rust scanner's approach: walk up the process tree
  # and ask xdotool for the window owned by each ancestor PID.
  if [ -z "${focus_id}" ] && [[ "$uname_s" == "Linux" ]] && command -v xdotool >/dev/null 2>&1; then
    local walk_pid="${PPID}"
    local i
    for i in 1 2 3 4 5 6; do
      [ -z "$walk_pid" ] || [ "$walk_pid" = "1" ] || [ "$walk_pid" = "0" ] && break
      local wid
      wid=$(xdotool search --pid "$walk_pid" 2>/dev/null | head -1 || true)
      if [ -n "$wid" ]; then
        focus_id=$(printf '0x%x' "$wid")
        kind="x11_generic"
        # Try to identify the terminal app from WM_CLASS
        if command -v xprop >/dev/null 2>&1; then
          local wm_class
          wm_class=$(xprop -id "$wid" WM_CLASS 2>/dev/null | awk -F'"' '{print $(NF-1)}' || true)
          if [ -n "$wm_class" ]; then
            label="$wm_class"
          fi
        fi
        break
      fi
      walk_pid=$(ps -o ppid= -p "$walk_pid" 2>/dev/null | tr -d ' ' || true)
    done
  fi

  printf '{"kind":"%s","focus_id":"%s","outer_id":"%s","label":"%s","window_title":"%s"}' \
    "${kind}" "${focus_id}" "${outer_id}" "${label}" "${window_title}"
}

TERMINAL_JSON=$(build_terminal_json)

# ── Status File Writer ─────────────────────────────────────────

write_status() {
  local status="$1" message="$2" hook_event="$3" hook_matcher="${4:-}"

  # Truncate message
  message="${message:0:500}"
  local safe_msg safe_cwd
  safe_msg=$(json_str "$message")
  safe_cwd=$(json_str "$CWD")

  cat > "${STATUS_FILE}.tmp" <<EOJSON
{"v":1,"status":"${status}","message":${safe_msg},"cwd":${safe_cwd},"source":"hook","cli":"${CLI}","session_id":"${SESSION_ID}","hook_event":"${hook_event}","hook_matcher":"${hook_matcher}","terminal":${TERMINAL_JSON}}
EOJSON
  mv -f "${STATUS_FILE}.tmp" "${STATUS_FILE}"
}

delete_status() {
  rm -f "${STATUS_FILE}" "${STATUS_FILE}.tmp"
}

# ── Event Mapping ──────────────────────────────────────────────

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
          write_status "idle" "Waiting for input" "$EVENT" "$MATCHER"
          ;;
        elicitation_dialog)
          write_status "needs-input" "MCP input requested" "$EVENT" "$MATCHER"
          ;;
        auth_success)
          # Auth succeeded — not user-actionable, ignore
          ;;
        *)
          write_status "needs-input" "Notification: ${MATCHER}" "$EVENT" "$MATCHER"
          ;;
      esac
      ;;
    Stop)
      write_status "idle" "Waiting for input" "$EVENT"
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
      # Only delete from the root session; a subagent SessionEnd must not
      # remove the parent's status file.
      if [ "$IS_SUBAGENT" = "true" ]; then
        write_status "working" "Subagent ended" "$EVENT"
      else
        delete_status
      fi
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
