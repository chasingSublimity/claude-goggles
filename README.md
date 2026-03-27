# claude-goggles

A terminal UI that visualizes Claude Code agent and subagent activity in real-time.

When Claude Code spawns parallel subagents, there's no visibility into what they're doing until they report back. claude-goggles hooks into Claude Code's event system and renders a live agent tree showing status, tool calls, elapsed time, and token usage.

## Install

```bash
cargo install --path .
```

## Usage

```bash
# Install hooks into ~/.claude/settings.json
claude-goggles init

# Launch the TUI
claude-goggles

# Use the bloom visualization mode
claude-goggles --viz bloom

# Remove hooks and clean up
claude-goggles clean
```

`init` adds hook entries to your existing `~/.claude/settings.json`, preserving any hooks you already have configured. If claude-goggles hooks are already present, it skips them.

## How it works

Claude Code hooks pipe event JSON over a Unix Domain Socket (`~/.claude-goggles/goggles.sock`). The TUI listens on that socket, parses incoming events, updates an in-memory agent tree, and re-renders at ~60fps.

Five hook events are captured: `PreToolUse`, `PostToolUse`, `SubagentStart`, `SubagentStop`, and `Stop`.

The hooks are silent no-ops when the TUI isn't running — Claude Code is never affected.

## Keyboard controls

| Key | Action |
|-----|--------|
| `q` / `Ctrl+C` | Quit |
| `v` | Toggle visualization (tree / bloom) |
| `↑↓` / `j/k` | Scroll (tree mode) |
| `c` | Collapse/expand subtrees (tree mode) |
| `[` / `]` | Cycle bloom parameter |
| `+` / `-` | Adjust selected parameter |
| `r` | Reset bloom parameters to defaults |

## License

BSD 3-Clause
