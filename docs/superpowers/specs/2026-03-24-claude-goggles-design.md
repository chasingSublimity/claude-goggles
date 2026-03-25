# claude-goggles Design Spec

A Rust TUI that visualizes Claude Code agent and subagent activity in real-time.

## Problem

When Claude Code spawns parallel subagents, there is no visibility into what they are doing until they report back. Users have no way to monitor the agent tree, see which tools are being called, or track resource usage across agents during a session.

## Solution

A terminal-based live monitor that receives events from Claude Code via hooks and renders an agent tree with per-agent status, current tool calls, elapsed time, and token usage.

## Architecture

### System Overview

Three components, all packaged in a single binary:

1. **Claude Code hooks** — shell one-liners that pipe event JSON over a Unix Domain Socket
2. **Event daemon** — UDS listener that parses incoming hook events and updates the in-memory model
3. **TUI renderer** — ratatui-based UI that renders the agent tree at ~10fps

### Transport: Unix Domain Socket

Socket path: `~/.claude-goggles/goggles.sock`

Hooks are one-liner shell commands installed into `~/.claude/settings.json`:

```json
{
  "hooks": {
    "PreToolUse": [{ "command": "cat | nc -U ~/.claude-goggles/goggles.sock 2>/dev/null || true" }],
    "PostToolUse": [{ "command": "cat | nc -U ~/.claude-goggles/goggles.sock 2>/dev/null || true" }],
    "SubagentStart": [{ "command": "cat | nc -U ~/.claude-goggles/goggles.sock 2>/dev/null || true" }],
    "SubagentStop": [{ "command": "cat | nc -U ~/.claude-goggles/goggles.sock 2>/dev/null || true" }],
    "Stop": [{ "command": "cat | nc -U ~/.claude-goggles/goggles.sock 2>/dev/null || true" }]
  }
}
```

**Connection lifecycle:** Each hook invocation opens a new UDS connection, writes the JSON payload, and closes when stdin reaches EOF. BSD `nc` (macOS) closes automatically after stdin EOF; GNU `nc` / `ncat` (Linux) also closes after piped input ends. The daemon accepts one connection per event, reads until EOF, closes its end, and parses the result. This is a connection-per-event model — the daemon must handle rapid short-lived connections from parallel agents.

**Silent failure:** `2>/dev/null || true` ensures the hook always exits 0, even if the socket doesn't exist (TUI not running) or `nc` fails. Claude Code continues unaffected.

**Platform note:** The `nc` (netcat) command varies across platforms. macOS ships BSD `nc` which supports `-U` natively and closes after piped stdin EOF. On Linux, `nc` may be `ncat`, `netcat-openbsd`, or GNU netcat — not all support `-U`. The `claude-goggles init` command will detect the available variant and use `socat - UNIX-CONNECT:path` as a fallback.

Why UDS over alternatives:
- Zero latency, no serialization overhead beyond JSON
- No port conflicts — uses a file path, not a network port
- Hooks are trivial one-liners
- Natural fit for a local-only tool

### Hook Events

Five hook types are captured:

| Hook | Purpose | Key Fields Used |
|------|---------|-----------------|
| `PreToolUse` | Agent is about to call a tool | `session_id`, `agent_id?`, `tool_name`, `tool_input`, `tool_use_id` |
| `PostToolUse` | Tool call completed | `session_id`, `agent_id?`, `tool_name`, `tool_input`, `tool_response`, `tool_use_id` |
| `SubagentStart` | Subagent spawned | `session_id`, `agent_id`, `agent_type` |
| `SubagentStop` | Subagent finished | `session_id`, `agent_id`, `agent_type`, `last_assistant_message` |
| `Stop` | Main agent turn ended | `session_id`, `stop_hook_active`, `last_assistant_message` |

All hook events share a common base: `{ session_id, transcript_path, cwd, hook_event_name }`. The `agent_id` field is present when the event fires inside a subagent context (added in Claude Code v2.1.69). Events from the main agent have no `agent_id`.

**Subagent task description:** When `tool_name` is `"Agent"`, the `tool_input` contains `{ description, prompt, subagent_type }`. We capture `description` as the subagent's task label by correlating the PreToolUse Agent call with the subsequent SubagentStart event.

**Token usage:** Not currently available in hook events (open feature request anthropics/claude-code#11008). For v1, token usage columns will show "—" as a placeholder. When the feature ships, we add parsing with no architectural changes needed.

**Parent-child inference:** There is no `parent_id` field in hook events (open request anthropics/claude-code#14859). We infer the tree structure by tracking which agent issued the PreToolUse with `tool_name: "Agent"` — that agent is the parent of the next SubagentStart. Events without `agent_id` belong to the main/root agent.

Hooks forward raw JSON from Claude Code — no transformation in the hook scripts. All parsing happens in the TUI binary.

### Data Model

```
Session
 └── Agent
      ├── id: String
      ├── task: String (description of what it was spawned to do)
      ├── status: Idle | Running(tool_name, key_arg) | Completed
      ├── started_at: Instant
      ├── finished_at: Option<Instant>
      ├── token_usage: Option<TokenUsage> (None until hook API supports it)
      ├── tool_history: Vec<ToolCall>
      └── children: Vec<Agent>
```

Event application is pure logic (no IO):

- **PreToolUse** → find agent by ID (or root if no `agent_id`), set status to `Running(tool_name, key_arg)`. The `key_arg` is extracted from `tool_input` by tool type: `file_path` for Read/Write/Edit, `command` (truncated) for Bash, `pattern` for Grep/Glob, `prompt` (truncated) for Agent, and `tool_name` as fallback for unknown tools. If `tool_name` is `"Agent"`, also record `tool_input.description` and `tool_use_id` in a per-agent pending spawn map (keyed by agent ID to handle concurrent spawns safely).
- **PostToolUse** → find agent, set status to `Idle`, push to `tool_history`
- **SubagentStart** → insert new child Agent under the parent that issued the pending Agent tool call. Set `task` from the recorded description.
- **SubagentStop** → find agent by `agent_id`, set status to `Completed`, record `finished_at`
- **Stop** → set root agent status to `Completed`

**Malformed events:** Invalid JSON or events missing required fields are silently dropped. A debug counter in the footer shows dropped event count if nonzero.

### Module Architecture

```
src/
  main.rs          — CLI arg parsing, starts socket + TUI
  events/
    mod.rs         — hook event types, JSON deserialization
    socket.rs      — UDS listener, accepts connections, parses events
  model/
    mod.rs         — AgentTree, Agent, ToolCall, TokenUsage
    update.rs      — applies events to the tree (pure logic, no IO)
  render/
    mod.rs         — Renderer trait
    tree_view.rs   — layout A implementation (htop-style tree list)
  cli/
    mod.rs         — init/clean commands, arg parsing
```

Critical boundary: **`render/` only depends on `model/`**. It never touches `events/` or the socket. The `Renderer` trait takes an `&AgentTree` and a `&mut Frame` and draws it. This enables swapping renderers without touching data or event logic.

`model/update.rs` is pure functions: `fn apply_event(tree: &mut AgentTree, event: HookEvent)`. No async, no IO — easy to test.

### Main Loop

1. **Async task:** socket listener accepts connections, reads each to EOF, parses JSON, sends parsed events through a `tokio::sync::mpsc` channel
2. **Main thread:** fixed 100ms tick timer. On each tick: drain all pending events from channel, apply each to model, re-render. This gives ~10fps with batched event processing.

### TUI Layout (Layout A — Tree List)

Single scrollable list with indentation showing agent hierarchy. Each row shows:

```
● agent-name ─ Task description
  │ ToolName key/arg · 1m 45s · —
```

- Green `●` for active agents, grey `◯` for completed
- Tool name highlighted, key argument dimmed
- Duration and token count right-aligned
- Tree lines (`├─`, `└─`, `│`) showing parent-child relationships

Footer bar: agent count, total tokens, keyboard shortcuts.

Keyboard controls:
- `q` / `Ctrl+C` — quit
- `↑↓` / `j/k` — scroll
- `c` — collapse/expand subtrees

### CLI Interface

```
claude-goggles          # launch the TUI
claude-goggles init     # install hooks into ~/.claude/settings.json
claude-goggles clean    # remove hooks, delete socket
```

`init` reads existing `~/.claude/settings.json`, parses it, and appends hook entries to each hook type's array (preserving any existing hooks the user has configured). If a claude-goggles hook is already present, it skips that entry. Writes the file back. Creates `~/.claude-goggles/` if needed. Detects whether `nc -U` or `socat` is available and uses the appropriate command in the hook.

## Edge Cases

- **TUI not running:** hook command exits 0 due to `|| true`, Claude Code unaffected
- **Multiple sessions:** events from all concurrent Claude Code sessions are mixed into one view (filter by session ID is a future enhancement)
- **Mid-session start:** only sees agents from TUI launch onward, no backfill
- **Socket cleanup:** TUI removes socket on clean exit; on startup, checks for and removes stale socket files
- **Crash recovery:** stale socket detected by attempting to connect — if connection refused, socket is stale and safe to remove

## Dependencies

| Crate | Purpose |
|-------|---------|
| `ratatui` | Terminal UI framework |
| `crossterm` | Terminal backend for ratatui |
| `tokio` | Async runtime for socket listener |
| `serde` / `serde_json` | JSON deserialization of hook events |
| `clap` | CLI argument parsing |

## Future Direction

The rendering layer is intentionally decoupled to support future visual experimentation. The long-term vision is an abstract, generative-art aesthetic rather than a traditional monitoring dashboard. The `Renderer` trait boundary enables this evolution without touching event processing or data modeling.
