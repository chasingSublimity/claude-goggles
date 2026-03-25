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
    "PreToolUse": [{ "command": "cat | nc -U ~/.claude-goggles/goggles.sock" }],
    "PostToolUse": [{ "command": "cat | nc -U ~/.claude-goggles/goggles.sock" }],
    "Notification": [{ "command": "cat | nc -U ~/.claude-goggles/goggles.sock" }],
    "Stop": [{ "command": "cat | nc -U ~/.claude-goggles/goggles.sock" }]
  }
}
```

Why UDS over alternatives:
- Zero latency, no serialization overhead beyond JSON
- No port conflicts — uses a file path, not a network port
- Hooks are trivial one-liners
- If the TUI isn't running, `nc` fails silently and Claude Code continues unaffected

### Hook Events

Four hook types are captured:

| Hook | Purpose | Key Data |
|------|---------|----------|
| `PreToolUse` | Agent is about to call a tool | agent ID, tool name, key arg (e.g., file path) |
| `PostToolUse` | Tool call completed | agent ID, tool name, duration |
| `Notification` | Subagent lifecycle | agent spawn, agent completion |
| `Stop` | Agent finished | agent ID, final status |

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
      ├── token_usage: TokenUsage { input: u64, output: u64 }
      ├── tool_history: Vec<ToolCall>
      └── children: Vec<Agent>
```

Event application is pure logic (no IO):

- **PreToolUse** → find agent by ID, set status to `Running("Read", "src/main.rs")`
- **PostToolUse** → set status to `Idle`, record duration, update token counts
- **Notification (subagent spawn)** → insert new child Agent under parent
- **Stop** → set status to `Completed`, record `finished_at`

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

1. **Async task:** socket listener receives events, sends through a `tokio::sync::mpsc` channel
2. **Main thread:** pulls events from channel, applies to model, re-renders on every tick (~10fps)

### TUI Layout (Layout A — Tree List)

Single scrollable list with indentation showing agent hierarchy. Each row shows:

```
● agent-name ─ Task description
  │ ToolName key/arg · 1m 45s · 3.1k tok
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

`init` reads existing settings, merges hook entries (preserving other hooks), writes back. Creates `~/.claude-goggles/` if needed.

## Edge Cases

- **TUI not running:** `nc` fails silently, Claude Code unaffected
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
