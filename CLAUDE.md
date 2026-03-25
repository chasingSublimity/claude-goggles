# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test

```bash
cargo build              # build debug
cargo build --release    # build release
cargo test               # run all tests
cargo test model         # run model tests
cargo test events        # run event parsing tests
cargo test cli           # run CLI tests
cargo test transcript    # run transcript parser tests
```

## Architecture

Single-binary Rust TUI that visualizes Claude Code agent activity in real-time.

**Data flow:** Claude Code hooks → UDS (`~/.claude-goggles/goggles.sock`) → async socket listener → mpsc channel → model update → TUI render

Four modules with strict dependency boundaries:
- `events/` — hook event JSON parsing, UDS socket listener, transcript JSONL parser
- `model/` — AgentTree data structure and pure event application logic (no IO)
- `render/` — Renderer trait + implementations. **Only depends on `model/`**. Decoupled for future visual experimentation.
- `cli/` — `init` (install hooks) and `clean` (remove hooks) commands

The `render/` ↔ `model/` boundary is critical — rendering must never depend on events or IO. This enables swapping renderers without touching business logic.

## Design Spec

See `docs/superpowers/specs/2026-03-24-claude-goggles-design.md` for full architecture decisions and rationale.
