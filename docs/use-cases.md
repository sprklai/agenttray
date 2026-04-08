# AgentTray Use Cases

These are the situations AgentTray is meant to help with. If your use case looks very different, the project probably still needs work.

## 1. Long Codex Run While You Work Elsewhere

You kick off a Codex task, switch to your editor, browser, or Slack, and stop watching the terminal.

What you get:

- the tray stays visible even when the terminal does not
- the status changes from `working` to `needs-input` when Codex pauses
- the popup gives you a direct path back to the relevant session when focus metadata is available

Best setup:

- install Codex hooks with `./scripts/hooks/install-hooks.sh codex`
- keep AgentTray running in the background

## 2. Claude Code Waiting on Permission or Input

Claude Code is especially useful when it can run for a while without supervision, but that only works if you notice when it stops to ask something.

What you get:

- permission prompts map to `needs-input`
- the tray color changes immediately instead of relying on one transient notification
- the dashboard shows which session is waiting and what kind of pause happened

Best setup:

- install Claude hooks with `./scripts/hooks/install-hooks.sh claude`
- verify that `~/.agent-monitor/` starts receiving `.status` files during a session

## 3. Multiple Agent Sessions Across Terminals

If you run more than one AI session at a time, it is easy to lose track of which terminal still needs attention.

What you get:

- one popup list for hook-based and scan-based sessions
- per-session status, short message, and terminal label
- a focus action when the terminal or multiplexer exposes enough metadata

Best setup:

- install hooks for the CLIs you use most
- use a supported terminal or multiplexer if you want reliable focus behavior

## 4. Unsupported CLI or Custom Wrapper

Not every CLI has a hook API yet. Some teams also run custom agent wrappers that will never match the built-in integrations exactly.

What you get:

- a file-based fallback path through `scripts/wrap.sh` or `scripts/wrap.ps1`
- no need to wait for built-in support before testing the tray setup

Example:

```bash
./scripts/wrap.sh my-agent -- my-custom-agent --project foo
```

## What These Use Cases Have in Common

AgentTray is most useful when:

- your agent sessions are long enough to outlive your immediate attention
- you would rather watch one tray signal than chase notifications
- you want a local desktop utility, not a hosted dashboard
