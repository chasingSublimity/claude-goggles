# claude-goggles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust TUI that visualizes Claude Code agent/subagent activity in real-time via hooks and a Unix Domain Socket.

**Architecture:** Single binary with four modules: `events/` (UDS listener + JSON parsing), `model/` (agent tree + pure event application), `render/` (trait-based TUI rendering decoupled from data), `cli/` (hook installation). Events flow from hooks → UDS → channel → model → renderer at ~10fps.

**Tech Stack:** Rust, ratatui, crossterm, tokio, serde/serde_json, clap

**Spec:** `docs/superpowers/specs/2026-03-24-claude-goggles-design.md`

---

## File Structure

```
Cargo.toml
src/
  main.rs              — CLI dispatch (clap), launches TUI or runs init/clean
  events/
    mod.rs             — HookEvent enum, JSON deserialization, key_arg extraction
    socket.rs          — UDS listener, async accept loop, sends parsed events via channel
    transcript.rs      — parse_transcript_usage(): reads JSONL transcript, sums token usage
  model/
    mod.rs             — AgentTree, Agent, AgentStatus, ToolCall, TokenUsage structs
    update.rs          — apply_event(): pure function mapping HookEvent → AgentTree mutations
  render/
    mod.rs             — Renderer trait definition
    tree_view.rs       — Layout A implementation: htop-style tree list with ratatui
  cli/
    mod.rs             — init (install hooks), clean (remove hooks + socket)
```

---

### Task 1: Project Scaffold + Cargo.toml

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `.gitignore`

- [ ] **Step 1: Initialize the Cargo project**

Run: `cargo init --name claude-goggles`

- [ ] **Step 2: Add dependencies to Cargo.toml**

```toml
[package]
name = "claude-goggles"
version = "0.1.0"
edition = "2021"

[dependencies]
ratatui = "0.29"
crossterm = "0.28"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
```

- [ ] **Step 3: Write minimal main.rs**

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "claude-goggles", about = "Visualize Claude Code agent activity")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Install hooks into ~/.claude/settings.json
    Init,
    /// Remove hooks and clean up socket
    Clean,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Init) => {
            println!("TODO: init");
        }
        Some(Commands::Clean) => {
            println!("TODO: clean");
        }
        None => {
            println!("TODO: launch TUI");
        }
    }
}
```

- [ ] **Step 4: Verify it builds and runs**

Run: `cargo build`
Expected: compiles with no errors

Run: `cargo run`
Expected: prints "TODO: launch TUI"

Run: `cargo run -- init`
Expected: prints "TODO: init"

- [ ] **Step 5: Add .superpowers/ to .gitignore**

Append `.superpowers/` and `target/` to `.gitignore`.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs .gitignore
git commit -m "feat: scaffold project with clap CLI"
```

---

### Task 2: Data Model (`model/`)

**Files:**
- Create: `src/model/mod.rs`
- Create: `src/model/update.rs`
- Modify: `src/main.rs` (add `mod model;`)

- [ ] **Step 1: Write tests for the data model**

Create `src/model/mod.rs` with the core types and inline tests:

```rust
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub tool_name: String,
    pub key_arg: String,
}

#[derive(Debug, Clone)]
pub enum AgentStatus {
    Idle,
    Running { tool_name: String, key_arg: String },
    Completed,
}

#[derive(Debug, Clone)]
pub struct Agent {
    pub id: String,
    pub task: String,
    pub status: AgentStatus,
    pub started_at: Instant,
    pub finished_at: Option<Instant>,
    pub token_usage: Option<TokenUsage>,
    pub tool_history: Vec<ToolCall>,
    pub children: Vec<Agent>,
    pub collapsed: bool,
}

impl Agent {
    pub fn new(id: String, task: String) -> Self {
        Self {
            id,
            task,
            status: AgentStatus::Idle,
            started_at: Instant::now(),
            finished_at: None,
            token_usage: None,
            tool_history: Vec::new(),
            children: Vec::new(),
            collapsed: false,
        }
    }

    /// Find a mutable reference to an agent by ID, searching recursively.
    pub fn find_mut(&mut self, id: &str) -> Option<&mut Agent> {
        if self.id == id {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_mut(id) {
                return Some(found);
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct AgentTree {
    pub session_id: Option<String>,
    pub root: Option<Agent>,
    pub dropped_events: u64,
    /// Maps parent agent ID → Vec<(tool_use_id, description)> for pending Agent tool calls
    pub pending_spawns: std::collections::HashMap<String, Vec<(String, String)>>,
}

impl AgentTree {
    pub fn new() -> Self {
        Self {
            session_id: None,
            root: None,
            dropped_events: 0,
            pending_spawns: std::collections::HashMap::new(),
        }
    }

    pub fn find_agent_mut(&mut self, agent_id: Option<&str>) -> Option<&mut Agent> {
        let root = self.root.as_mut()?;
        match agent_id {
            None => Some(root),
            Some(id) => root.find_mut(id),
        }
    }
}

pub mod update;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_new() {
        let agent = Agent::new("test-1".into(), "Run tests".into());
        assert_eq!(agent.id, "test-1");
        assert_eq!(agent.task, "Run tests");
        assert!(matches!(agent.status, AgentStatus::Idle));
        assert!(agent.finished_at.is_none());
        assert!(agent.token_usage.is_none());
        assert!(agent.children.is_empty());
    }

    #[test]
    fn test_find_mut_root() {
        let mut agent = Agent::new("root".into(), "Main".into());
        assert!(agent.find_mut("root").is_some());
        assert!(agent.find_mut("nonexistent").is_none());
    }

    #[test]
    fn test_find_mut_nested() {
        let mut root = Agent::new("root".into(), "Main".into());
        let child = Agent::new("child-1".into(), "Sub task".into());
        root.children.push(child);

        assert!(root.find_mut("child-1").is_some());
        assert_eq!(root.find_mut("child-1").unwrap().task, "Sub task");
    }

    #[test]
    fn test_agent_tree_find_agent_none_returns_root() {
        let mut tree = AgentTree::new();
        tree.root = Some(Agent::new("root".into(), "Main".into()));
        assert!(tree.find_agent_mut(None).is_some());
        assert_eq!(tree.find_agent_mut(None).unwrap().id, "root");
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test model`
Expected: all 4 tests pass

- [ ] **Step 3: Write tests for event application**

Create `src/model/update.rs`:

```rust
use super::{Agent, AgentStatus, AgentTree, ToolCall, TokenUsage};
use crate::events::HookEvent;

/// Apply a hook event to the agent tree. Pure function — no IO.
pub fn apply_event(tree: &mut AgentTree, event: HookEvent) {
    match event {
        HookEvent::PreToolUse {
            session_id,
            agent_id,
            tool_name,
            key_arg,
            tool_use_id,
            spawns_agent,
        } => {
            ensure_root(tree, &session_id);
            if spawns_agent.is_some() {
                let parent_id = agent_id.clone().unwrap_or_else(|| "root".into());
                let (desc,) = spawns_agent.unwrap();
                tree.pending_spawns
                    .entry(parent_id)
                    .or_default()
                    .push((tool_use_id, desc));
            }
            if let Some(agent) = tree.find_agent_mut(agent_id.as_deref()) {
                agent.status = AgentStatus::Running { tool_name, key_arg };
            }
        }
        HookEvent::PostToolUse {
            session_id,
            agent_id,
            tool_name,
            key_arg,
            ..
        } => {
            ensure_root(tree, &session_id);
            if let Some(agent) = tree.find_agent_mut(agent_id.as_deref()) {
                agent.tool_history.push(ToolCall { tool_name, key_arg });
                agent.status = AgentStatus::Idle;
            }
        }
        HookEvent::SubagentStart {
            session_id,
            agent_id,
            agent_type,
        } => {
            ensure_root(tree, &session_id);
            // Find which parent has a pending spawn and pop the first one
            let mut parent_key = None;
            let mut task_desc = agent_type.clone();
            for (pid, pending) in &tree.pending_spawns {
                if !pending.is_empty() {
                    parent_key = Some(pid.clone());
                    task_desc = pending[0].1.clone();
                    break;
                }
            }
            if let Some(pk) = &parent_key {
                if let Some(pending) = tree.pending_spawns.get_mut(pk) {
                    pending.remove(0);
                }
            }
            let child = Agent::new(agent_id, task_desc);
            let parent_id = parent_key.as_deref();
            if let Some(parent) = tree.find_agent_mut(parent_id) {
                parent.children.push(child);
            }
        }
        HookEvent::SubagentStop {
            agent_id,
            token_usage,
            ..
        } => {
            if let Some(root) = &mut tree.root {
                if let Some(agent) = root.find_mut(&agent_id) {
                    agent.status = AgentStatus::Completed;
                    agent.finished_at = Some(std::time::Instant::now());
                    agent.token_usage = token_usage;
                }
            }
        }
        HookEvent::Stop { token_usage, .. } => {
            if let Some(root) = &mut tree.root {
                root.status = AgentStatus::Completed;
                root.finished_at = Some(std::time::Instant::now());
                root.token_usage = token_usage;
            }
        }
    }
}

fn ensure_root(tree: &mut AgentTree, session_id: &str) {
    if tree.root.is_none() {
        tree.session_id = Some(session_id.to_string());
        tree.root = Some(Agent::new("root".into(), "Main session".into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pre_tool_use(agent_id: Option<&str>, tool: &str, arg: &str) -> HookEvent {
        HookEvent::PreToolUse {
            session_id: "sess-1".into(),
            agent_id: agent_id.map(String::from),
            tool_name: tool.into(),
            key_arg: arg.into(),
            tool_use_id: "tu-1".into(),
            spawns_agent: None,
        }
    }

    fn make_post_tool_use(agent_id: Option<&str>, tool: &str, arg: &str) -> HookEvent {
        HookEvent::PostToolUse {
            session_id: "sess-1".into(),
            agent_id: agent_id.map(String::from),
            tool_name: tool.into(),
            key_arg: arg.into(),
            tool_use_id: "tu-1".into(),
        }
    }

    #[test]
    fn test_pre_tool_use_creates_root_and_sets_running() {
        let mut tree = AgentTree::new();
        apply_event(&mut tree, make_pre_tool_use(None, "Read", "src/main.rs"));

        let root = tree.root.as_ref().unwrap();
        assert!(matches!(&root.status, AgentStatus::Running { tool_name, .. } if tool_name == "Read"));
    }

    #[test]
    fn test_post_tool_use_sets_idle_and_records_history() {
        let mut tree = AgentTree::new();
        apply_event(&mut tree, make_pre_tool_use(None, "Read", "src/main.rs"));
        apply_event(&mut tree, make_post_tool_use(None, "Read", "src/main.rs"));

        let root = tree.root.as_ref().unwrap();
        assert!(matches!(root.status, AgentStatus::Idle));
        assert_eq!(root.tool_history.len(), 1);
        assert_eq!(root.tool_history[0].tool_name, "Read");
    }

    #[test]
    fn test_subagent_lifecycle() {
        let mut tree = AgentTree::new();
        // Parent spawns an agent
        apply_event(
            &mut tree,
            HookEvent::PreToolUse {
                session_id: "sess-1".into(),
                agent_id: None,
                tool_name: "Agent".into(),
                key_arg: "Explore codebase".into(),
                tool_use_id: "tu-agent-1".into(),
                spawns_agent: Some(("Explore codebase".into(),)),
            },
        );
        // Subagent starts
        apply_event(
            &mut tree,
            HookEvent::SubagentStart {
                session_id: "sess-1".into(),
                agent_id: "agent-1".into(),
                agent_type: "Explore".into(),
            },
        );

        let root = tree.root.as_ref().unwrap();
        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].id, "agent-1");
        assert_eq!(root.children[0].task, "Explore codebase");

        // Subagent stops
        apply_event(
            &mut tree,
            HookEvent::SubagentStop {
                session_id: "sess-1".into(),
                agent_id: "agent-1".into(),
                agent_type: "Explore".into(),
                token_usage: Some(TokenUsage {
                    input: 1000,
                    output: 500,
                }),
            },
        );

        let root = tree.root.as_ref().unwrap();
        let child = &root.children[0];
        assert!(matches!(child.status, AgentStatus::Completed));
        assert!(child.token_usage.is_some());
        assert_eq!(child.token_usage.as_ref().unwrap().input, 1000);
    }

    #[test]
    fn test_stop_completes_root() {
        let mut tree = AgentTree::new();
        apply_event(&mut tree, make_pre_tool_use(None, "Read", "file.rs"));
        apply_event(
            &mut tree,
            HookEvent::Stop {
                session_id: "sess-1".into(),
                token_usage: Some(TokenUsage {
                    input: 5000,
                    output: 2000,
                }),
            },
        );

        let root = tree.root.as_ref().unwrap();
        assert!(matches!(root.status, AgentStatus::Completed));
        assert_eq!(root.token_usage.as_ref().unwrap().input, 5000);
    }

    #[test]
    fn test_concurrent_spawns_from_same_parent() {
        let mut tree = AgentTree::new();
        // Parent spawns two agents
        apply_event(
            &mut tree,
            HookEvent::PreToolUse {
                session_id: "sess-1".into(),
                agent_id: None,
                tool_name: "Agent".into(),
                key_arg: "Write tests".into(),
                tool_use_id: "tu-a1".into(),
                spawns_agent: Some(("Write tests".into(),)),
            },
        );
        apply_event(
            &mut tree,
            HookEvent::PreToolUse {
                session_id: "sess-1".into(),
                agent_id: None,
                tool_name: "Agent".into(),
                key_arg: "Update docs".into(),
                tool_use_id: "tu-a2".into(),
                spawns_agent: Some(("Update docs".into(),)),
            },
        );
        // Both start
        apply_event(
            &mut tree,
            HookEvent::SubagentStart {
                session_id: "sess-1".into(),
                agent_id: "agent-1".into(),
                agent_type: "general-purpose".into(),
            },
        );
        apply_event(
            &mut tree,
            HookEvent::SubagentStart {
                session_id: "sess-1".into(),
                agent_id: "agent-2".into(),
                agent_type: "general-purpose".into(),
            },
        );

        let root = tree.root.as_ref().unwrap();
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].task, "Write tests");
        assert_eq!(root.children[1].task, "Update docs");
    }
}
```

- [ ] **Step 4: Add mod declarations to main.rs**

Add `mod model;` and a stub `mod events;` to `src/main.rs`. Create stub `src/events/mod.rs` with the `HookEvent` enum (no deserialization yet — just the enum definition needed by `model/update.rs`):

```rust
// src/events/mod.rs
use crate::model::TokenUsage;

#[derive(Debug)]
pub enum HookEvent {
    PreToolUse {
        session_id: String,
        agent_id: Option<String>,
        tool_name: String,
        key_arg: String,
        tool_use_id: String,
        /// If this is an Agent tool call: (description,)
        spawns_agent: Option<(String,)>,
    },
    PostToolUse {
        session_id: String,
        agent_id: Option<String>,
        tool_name: String,
        key_arg: String,
        tool_use_id: String,
    },
    SubagentStart {
        session_id: String,
        agent_id: String,
        agent_type: String,
    },
    SubagentStop {
        session_id: String,
        agent_id: String,
        agent_type: String,
        token_usage: Option<TokenUsage>,
    },
    Stop {
        session_id: String,
        token_usage: Option<TokenUsage>,
    },
}
```

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: all model tests pass

- [ ] **Step 6: Commit**

```bash
git add src/model/ src/events/mod.rs src/main.rs
git commit -m "feat: add data model and event application logic with tests"
```

---

### Task 3: Event Parsing — JSON Deserialization (`events/mod.rs`)

**Files:**
- Modify: `src/events/mod.rs`
- Create: `src/events/transcript.rs`

- [ ] **Step 1: Write tests for JSON → HookEvent parsing**

Add a `parse_hook_event(json: &str) -> Option<HookEvent>` function and tests. The function takes raw JSON from a hook, extracts `hook_event_name` to determine the variant, then extracts the relevant fields. Test with realistic JSON payloads matching the Claude Code hook schema.

Key test cases:
- PreToolUse with `tool_name: "Read"` → key_arg is `tool_input.file_path`
- PreToolUse with `tool_name: "Bash"` → key_arg is `tool_input.command` (truncated to 60 chars)
- PreToolUse with `tool_name: "Agent"` → key_arg is `tool_input.description`, `spawns_agent` populated
- PostToolUse → basic parsing
- SubagentStart → basic parsing
- SubagentStop → basic parsing
- Stop → basic parsing
- Unknown `hook_event_name` → returns `None`
- Malformed JSON → returns `None`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pre_tool_use_read() {
        let json = r#"{
            "session_id": "sess-1",
            "hook_event_name": "PreToolUse",
            "tool_name": "Read",
            "tool_input": { "file_path": "/src/main.rs" },
            "tool_use_id": "tu-1"
        }"#;
        let event = parse_hook_event(json).unwrap();
        match event {
            HookEvent::PreToolUse { tool_name, key_arg, .. } => {
                assert_eq!(tool_name, "Read");
                assert_eq!(key_arg, "/src/main.rs");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_parse_pre_tool_use_agent_spawns() {
        let json = r#"{
            "session_id": "sess-1",
            "hook_event_name": "PreToolUse",
            "tool_name": "Agent",
            "tool_input": { "description": "Explore codebase", "prompt": "...", "subagent_type": "Explore" },
            "tool_use_id": "tu-2"
        }"#;
        let event = parse_hook_event(json).unwrap();
        match event {
            HookEvent::PreToolUse { spawns_agent, key_arg, .. } => {
                assert!(spawns_agent.is_some());
                assert_eq!(spawns_agent.unwrap().0, "Explore codebase");
                assert_eq!(key_arg, "Explore codebase");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_parse_bash_truncates_command() {
        let long_cmd = "a]".repeat(100);
        let json = format!(r#"{{
            "session_id": "s",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {{ "command": "{long_cmd}" }},
            "tool_use_id": "tu-3"
        }}"#);
        let event = parse_hook_event(&json).unwrap();
        match event {
            HookEvent::PreToolUse { key_arg, .. } => {
                assert!(key_arg.len() <= 63); // 60 + "..."
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_parse_unknown_event_returns_none() {
        let json = r#"{ "session_id": "s", "hook_event_name": "SessionStart" }"#;
        assert!(parse_hook_event(json).is_none());
    }

    #[test]
    fn test_parse_malformed_json_returns_none() {
        assert!(parse_hook_event("not json").is_none());
    }

    #[test]
    fn test_parse_subagent_stop() {
        let json = r#"{
            "session_id": "sess-1",
            "hook_event_name": "SubagentStop",
            "agent_id": "agent-1",
            "agent_type": "Explore",
            "agent_transcript_path": "/tmp/transcript.jsonl",
            "last_assistant_message": "Done"
        }"#;
        let event = parse_hook_event(json).unwrap();
        assert!(matches!(event, HookEvent::SubagentStop { .. }));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test events`
Expected: FAIL — `parse_hook_event` not defined

- [ ] **Step 3: Implement parse_hook_event**

Use `serde_json::Value` for flexible parsing (since different events have different shapes). Extract `hook_event_name` first to dispatch, then pull relevant fields per variant. Implement `extract_key_arg(tool_name: &str, tool_input: &Value) -> String` for the per-tool key arg extraction logic.

```rust
use serde_json::Value;
use crate::model::TokenUsage;

// NOTE: pub mod transcript; is added in Task 4, not here.

// ... (HookEvent enum stays as-is) ...

pub fn parse_hook_event(json: &str) -> Option<HookEvent> {
    let v: Value = serde_json::from_str(json).ok()?;
    let event_name = v.get("hook_event_name")?.as_str()?;
    let session_id = v.get("session_id")?.as_str()?.to_string();

    match event_name {
        "PreToolUse" => {
            let tool_name = v.get("tool_name")?.as_str()?.to_string();
            let tool_input = v.get("tool_input").cloned().unwrap_or(Value::Null);
            let tool_use_id = v.get("tool_use_id")?.as_str()?.to_string();
            let agent_id = v.get("agent_id").and_then(|v| v.as_str()).map(String::from);
            let key_arg = extract_key_arg(&tool_name, &tool_input);
            let spawns_agent = if tool_name == "Agent" {
                let desc = tool_input.get("description")?.as_str()?.to_string();
                Some((desc,))
            } else {
                None
            };
            Some(HookEvent::PreToolUse {
                session_id,
                agent_id,
                tool_name,
                key_arg,
                tool_use_id,
                spawns_agent,
            })
        }
        "PostToolUse" => {
            let tool_name = v.get("tool_name")?.as_str()?.to_string();
            let tool_input = v.get("tool_input").cloned().unwrap_or(Value::Null);
            let tool_use_id = v.get("tool_use_id")?.as_str()?.to_string();
            let agent_id = v.get("agent_id").and_then(|v| v.as_str()).map(String::from);
            let key_arg = extract_key_arg(&tool_name, &tool_input);
            Some(HookEvent::PostToolUse {
                session_id,
                agent_id,
                tool_name,
                key_arg,
                tool_use_id,
            })
        }
        "SubagentStart" => {
            let agent_id = v.get("agent_id")?.as_str()?.to_string();
            let agent_type = v.get("agent_type").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
            Some(HookEvent::SubagentStart {
                session_id,
                agent_id,
                agent_type,
            })
        }
        "SubagentStop" => {
            let agent_id = v.get("agent_id")?.as_str()?.to_string();
            let agent_type = v.get("agent_type").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
            // NOTE: transcript reading is added in Task 4. Until then, token_usage is None.
            Some(HookEvent::SubagentStop {
                session_id,
                agent_id,
                agent_type,
                token_usage: None,
            })
        }
        "Stop" => {
            // NOTE: transcript reading is added in Task 4. Until then, token_usage is None.
            Some(HookEvent::Stop {
                session_id,
                token_usage: None,
            })
        }
        _ => None,
    }
}

fn extract_key_arg(tool_name: &str, tool_input: &Value) -> String {
    let raw = match tool_name {
        "Read" | "Write" | "Edit" => tool_input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "Bash" => tool_input
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "Grep" | "Glob" => tool_input
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "Agent" => tool_input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        _ => tool_name.to_string(),
    };
    truncate(&raw, 60)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{}...", truncated)
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test events`
Expected: all event parsing tests pass

Note: SubagentStop test will pass even without a real transcript file — `parse_transcript_usage` will return `None` for nonexistent paths, and the test doesn't assert on token_usage.

- [ ] **Step 5: Commit**

```bash
git add src/events/
git commit -m "feat: add hook event JSON parsing with key_arg extraction"
```

---

### Task 4: Transcript Parsing (`events/transcript.rs`)

**Files:**
- Create: `src/events/transcript.rs`
- Modify: `src/events/mod.rs` (add `pub mod transcript;`)

- [ ] **Step 0: Add module declaration**

Add `pub mod transcript;` to `src/events/mod.rs` (needed for the SubagentStop/Stop parsing that references `transcript::parse_transcript_usage`).

- [ ] **Step 1: Write tests for transcript JSONL parsing**

```rust
use std::path::Path;
use crate::model::TokenUsage;

/// Read a JSONL transcript file and sum all usage fields.
/// Returns None if the file can't be read or contains no usage data.
pub fn parse_transcript_usage(path: &Path) -> Option<TokenUsage> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_transcript(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn test_parse_usage_from_transcript() {
        let content = r#"{"type":"assistant","message":{"usage":{"input_tokens":100,"output_tokens":50}}}
{"type":"assistant","message":{"usage":{"input_tokens":200,"output_tokens":100}}}
{"type":"user","message":"hello"}
"#;
        let f = write_temp_transcript(content);
        let usage = parse_transcript_usage(f.path()).unwrap();
        assert_eq!(usage.input, 300);
        assert_eq!(usage.output, 150);
    }

    #[test]
    fn test_parse_empty_transcript() {
        let f = write_temp_transcript("");
        assert!(parse_transcript_usage(f.path()).is_none());
    }

    #[test]
    fn test_parse_no_usage_fields() {
        let content = r#"{"type":"user","message":"hello"}
"#;
        let f = write_temp_transcript(content);
        assert!(parse_transcript_usage(f.path()).is_none());
    }

    #[test]
    fn test_parse_nonexistent_file() {
        assert!(parse_transcript_usage(Path::new("/nonexistent/path.jsonl")).is_none());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test transcript`
Expected: FAIL — `todo!()` panics

Note: add `tempfile` as a dev dependency first:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Implement parse_transcript_usage**

```rust
use std::fs;
use std::path::Path;
use serde_json::Value;
use crate::model::TokenUsage;

pub fn parse_transcript_usage(path: &Path) -> Option<TokenUsage> {
    let content = fs::read_to_string(path).ok()?;
    let mut input_total: u64 = 0;
    let mut output_total: u64 = 0;
    let mut found = false;

    for line in content.lines() {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            if let Some(usage) = v.pointer("/message/usage") {
                if let (Some(inp), Some(out)) = (
                    usage.get("input_tokens").and_then(|v| v.as_u64()),
                    usage.get("output_tokens").and_then(|v| v.as_u64()),
                ) {
                    input_total += inp;
                    output_total += out;
                    found = true;
                }
            }
        }
    }

    if found {
        Some(TokenUsage {
            input: input_total,
            output: output_total,
        })
    } else {
        None
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test transcript`
Expected: all 4 tests pass

- [ ] **Step 5: Wire up transcript parsing in parse_hook_event**

Update the `SubagentStop` and `Stop` arms in `parse_hook_event` (in `src/events/mod.rs`) to read the transcript. Replace the `token_usage: None` placeholders:

For `SubagentStop`:
```rust
            let transcript_path = v.get("agent_transcript_path").and_then(|v| v.as_str()).map(String::from);
            let token_usage = transcript_path
                .as_deref()
                .and_then(|p| transcript::parse_transcript_usage(std::path::Path::new(p)));
```

For `Stop`:
```rust
            let transcript_path = v.get("transcript_path").and_then(|v| v.as_str()).map(String::from);
            let token_usage = transcript_path
                .as_deref()
                .and_then(|p| transcript::parse_transcript_usage(std::path::Path::new(p)));
```

- [ ] **Step 6: Verify all tests pass**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 7: Commit**

```bash
git add src/events/transcript.rs src/events/mod.rs Cargo.toml
git commit -m "feat: add transcript JSONL parser for token usage"
```

---

### Task 5: UDS Socket Listener (`events/socket.rs`)

**Files:**
- Create: `src/events/socket.rs`
- Modify: `src/events/mod.rs` (add `pub mod socket;`)

- [ ] **Step 1: Write tests for the socket listener**

```rust
use std::path::PathBuf;
use tokio::net::UnixStream;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use crate::events::HookEvent;

pub struct SocketListener {
    path: PathBuf,
}

impl SocketListener {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Remove stale socket file if it exists
    pub fn cleanup_stale(&self) {
        if self.path.exists() {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    /// Start listening. Sends parsed HookEvents through the channel.
    /// Runs until the channel is closed.
    pub async fn listen(&self, tx: mpsc::Sender<HookEvent>) -> std::io::Result<()> {
        todo!()
    }
}

impl Drop for SocketListener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_socket_receives_event() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("test.sock");
        let listener = SocketListener::new(sock_path.clone());
        let (tx, mut rx) = mpsc::channel(100);

        let handle = tokio::spawn(async move {
            listener.listen(tx).await.unwrap();
        });

        // Give the listener time to bind
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Send a valid event
        let json = r#"{"session_id":"s1","hook_event_name":"PreToolUse","tool_name":"Read","tool_input":{"file_path":"test.rs"},"tool_use_id":"t1"}"#;
        let mut stream = UnixStream::connect(&sock_path).await.unwrap();
        stream.write_all(json.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let event = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            rx.recv(),
        ).await.unwrap().unwrap();

        assert!(matches!(event, HookEvent::PreToolUse { .. }));

        handle.abort();
    }

    #[tokio::test]
    async fn test_socket_drops_malformed_json() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("test2.sock");
        let listener = SocketListener::new(sock_path.clone());
        let (tx, mut rx) = mpsc::channel(100);

        let handle = tokio::spawn(async move {
            listener.listen(tx).await.unwrap();
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Send garbage
        let mut stream = UnixStream::connect(&sock_path).await.unwrap();
        stream.write_all(b"not json").await.unwrap();
        stream.shutdown().await.unwrap();

        // Send valid event after
        let json = r#"{"session_id":"s1","hook_event_name":"Stop","transcript_path":"/tmp/t.jsonl"}"#;
        let mut stream2 = UnixStream::connect(&sock_path).await.unwrap();
        stream2.write_all(json.as_bytes()).await.unwrap();
        stream2.shutdown().await.unwrap();

        let event = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            rx.recv(),
        ).await.unwrap().unwrap();

        // Should get Stop, not the malformed event
        assert!(matches!(event, HookEvent::Stop { .. }));

        handle.abort();
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test socket`
Expected: FAIL — `todo!()` panics

- [ ] **Step 3: Implement the socket listener**

```rust
pub async fn listen(&self, tx: mpsc::Sender<HookEvent>) -> std::io::Result<()> {
    self.cleanup_stale();
    if let Some(parent) = self.path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let listener = tokio::net::UnixListener::bind(&self.path)?;

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let tx = tx.clone();
                tokio::spawn(async move {
                    if let Err(_) = handle_connection(stream, tx).await {
                        // Connection error — silently drop
                    }
                });
            }
            Err(_) => continue,
        }
    }
}

async fn handle_connection(
    mut stream: tokio::net::UnixStream,
    tx: mpsc::Sender<HookEvent>,
) -> std::io::Result<()> {
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;
    let json = String::from_utf8_lossy(&buf);
    if let Some(event) = super::parse_hook_event(&json) {
        let _ = tx.send(event).await;
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test socket`
Expected: both socket tests pass

- [ ] **Step 5: Commit**

```bash
git add src/events/
git commit -m "feat: add async UDS socket listener"
```

---

### Task 6: Renderer Trait + Tree View (`render/`)

**Files:**
- Create: `src/render/mod.rs`
- Create: `src/render/tree_view.rs`
- Modify: `src/main.rs` (add `mod render;`)

- [ ] **Step 1: Define the Renderer trait**

```rust
// src/render/mod.rs
pub mod tree_view;

use ratatui::Frame;
use crate::model::AgentTree;

pub trait Renderer {
    fn render(&self, tree: &AgentTree, frame: &mut Frame, scroll_offset: usize);
}
```

- [ ] **Step 2: Implement TreeViewRenderer**

Create `src/render/tree_view.rs`. This is the htop-style tree list renderer. It walks the `AgentTree` recursively, building a list of `Line` widgets with indentation, status indicators, tool info, duration, and token counts.

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::time::Instant;
use crate::model::{Agent, AgentStatus, AgentTree};
use super::Renderer;

pub struct TreeViewRenderer;

impl Renderer for TreeViewRenderer {
    fn render(&self, tree: &AgentTree, frame: &mut Frame, scroll_offset: usize) {
        let area = frame.area();

        // Split into main area and footer
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);

        // Render agent tree
        let mut lines: Vec<Line> = Vec::new();
        if let Some(ref root) = tree.root {
            let session_label = tree.session_id.as_deref().unwrap_or("unknown");
            let elapsed = format_duration(root.started_at);
            lines.push(Line::from(vec![
                Span::styled(
                    format!("SESSION {} · {}", session_label.chars().take(8).collect::<String>(), elapsed),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            render_agent(&mut lines, root, "", true);
        } else {
            lines.push(Line::from(Span::styled(
                "Waiting for events...",
                Style::default().fg(Color::DarkGray),
            )));
        }

        let tree_widget = Paragraph::new(lines)
            .block(Block::default().borders(Borders::NONE))
            .wrap(Wrap { trim: false })
            .scroll((scroll_offset as u16, 0));
        frame.render_widget(tree_widget, chunks[0]);

        // Render footer
        let (active, total) = count_agents(tree);
        let footer = Line::from(vec![
            Span::styled(
                format!("agents: {} ({} active)", total, active),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("dropped: {}", tree.dropped_events),
                Style::default().fg(if tree.dropped_events > 0 {
                    Color::Yellow
                } else {
                    Color::DarkGray
                }),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled("q: quit  j/k: scroll  c: collapse", Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(Paragraph::new(footer), chunks[1]);
    }
}

fn render_agent(lines: &mut Vec<Line>, agent: &Agent, prefix: &str, is_last: bool) {
    let connector = if prefix.is_empty() { "" } else if is_last { "└─ " } else { "├─ " };
    let status_icon = match &agent.status {
        AgentStatus::Completed => Span::styled("◯ ", Style::default().fg(Color::DarkGray)),
        _ => Span::styled("● ", Style::default().fg(Color::Green)),
    };

    let elapsed = format_duration(agent.started_at);
    let tokens = match &agent.token_usage {
        Some(t) => format_tokens(t.input + t.output),
        None => "—".to_string(),
    };

    lines.push(Line::from(vec![
        Span::styled(prefix.to_string(), Style::default().fg(Color::DarkGray)),
        Span::styled(connector.to_string(), Style::default().fg(Color::DarkGray)),
        status_icon,
        Span::styled(format!("{} ", agent.id), Style::default().fg(Color::Cyan)),
        Span::styled("─ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            agent.task.clone(),
            if matches!(agent.status, AgentStatus::Completed) {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            },
        ),
    ]));

    // Tool status line
    let child_prefix = format!(
        "{}{}",
        prefix,
        if prefix.is_empty() { "" } else if is_last { "   " } else { "│  " }
    );

    let tool_line = match &agent.status {
        AgentStatus::Running { tool_name, key_arg } => {
            format!("{} {}", tool_name, key_arg)
        }
        AgentStatus::Completed => "done".to_string(),
        AgentStatus::Idle => "idle".to_string(),
    };

    lines.push(Line::from(vec![
        Span::styled(format!("{}  │ ", child_prefix), Style::default().fg(Color::DarkGray)),
        Span::styled(
            tool_line,
            match &agent.status {
                AgentStatus::Running { .. } => Style::default().fg(Color::Yellow),
                _ => Style::default().fg(Color::DarkGray),
            },
        ),
        Span::styled(format!(" · {} · {}", elapsed, tokens), Style::default().fg(Color::DarkGray)),
    ]));

    if !agent.collapsed {
        for (i, child) in agent.children.iter().enumerate() {
            let is_last_child = i == agent.children.len() - 1;
            render_agent(lines, child, &child_prefix, is_last_child);
        }
    }
}

fn format_duration(started: Instant) -> String {
    let secs = started.elapsed().as_secs();
    let mins = secs / 60;
    let secs = secs % 60;
    format!("{}m {:02}s", mins, secs)
}

fn format_tokens(total: u64) -> String {
    if total >= 1000 {
        format!("{:.1}k tok", total as f64 / 1000.0)
    } else {
        format!("{} tok", total)
    }
}

fn count_agents(tree: &AgentTree) -> (usize, usize) {
    match &tree.root {
        None => (0, 0),
        Some(root) => {
            let mut active = 0;
            let mut total = 0;
            count_recursive(root, &mut active, &mut total);
            (active, total)
        }
    }
}

fn count_recursive(agent: &Agent, active: &mut usize, total: &mut usize) {
    *total += 1;
    if !matches!(agent.status, AgentStatus::Completed) {
        *active += 1;
    }
    for child in &agent.children {
        count_recursive(child, active, total);
    }
}
```

- [ ] **Step 3: Add unit tests for helper functions**

Add tests at the bottom of `tree_view.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tokens_small() {
        assert_eq!(format_tokens(500), "500 tok");
    }

    #[test]
    fn test_format_tokens_thousands() {
        assert_eq!(format_tokens(3100), "3.1k tok");
    }

    #[test]
    fn test_count_agents_empty() {
        let tree = AgentTree::new();
        assert_eq!(count_agents(&tree), (0, 0));
    }

    #[test]
    fn test_count_agents_with_children() {
        let mut tree = AgentTree::new();
        let mut root = Agent::new("root".into(), "Main".into());
        root.children.push(Agent::new("c1".into(), "Task 1".into()));
        let mut c2 = Agent::new("c2".into(), "Task 2".into());
        c2.status = AgentStatus::Completed;
        root.children.push(c2);
        tree.root = Some(root);
        assert_eq!(count_agents(&tree), (2, 3)); // root + c1 active, c2 completed
    }
}
```

- [ ] **Step 4: Verify it compiles and tests pass**

Run: `cargo test render`
Expected: all 4 render tests pass

- [ ] **Step 5: Commit**

```bash
git add src/render/
git commit -m "feat: add Renderer trait and tree view implementation"
```

---

### Task 7: Main Loop — TUI + Socket Integration (`main.rs`)

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Implement the main TUI loop**

Replace the `None` arm of the match in `main.rs` with the full TUI + socket integration:

```rust
use std::path::PathBuf;
use std::time::Duration;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;
use tokio::sync::mpsc;

mod events;
mod model;
mod render;
mod cli;

use events::socket::SocketListener;
use model::AgentTree;
use model::update::apply_event;
use render::Renderer;
use render::tree_view::TreeViewRenderer;

#[derive(Parser)]
#[command(name = "claude-goggles", about = "Visualize Claude Code agent activity")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Clean,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Init) => cli::init()?,
        Some(Commands::Clean) => cli::clean()?,
        None => run_tui()?,
    }
    Ok(())
}

fn run_tui() -> anyhow::Result<()> {
    let sock_path = dirs::home_dir()
        .expect("no home dir")
        .join(".claude-goggles")
        .join("goggles.sock");

    let rt = tokio::runtime::Runtime::new()?;
    let (tx, mut rx) = mpsc::channel(1000);

    // Start socket listener in background
    let listener = SocketListener::new(sock_path);
    rt.spawn(async move {
        if let Err(e) = listener.listen(tx).await {
            eprintln!("Socket error: {}", e);
        }
    });

    // Setup terminal
    enable_raw_mode()?;
    std::io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let renderer = TreeViewRenderer;
    let mut tree = AgentTree::new();
    let mut scroll_offset: usize = 0;

    loop {
        // Drain events from socket
        while let Ok(event) = rx.try_recv() {
            apply_event(&mut tree, event);
        }

        // Render
        terminal.draw(|frame| {
            renderer.render(&tree, frame, scroll_offset);
        })?;

        // Handle input (100ms timeout = ~10fps)
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Up | KeyCode::Char('k') => {
                        scroll_offset = scroll_offset.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        scroll_offset += 1;
                    }
                    KeyCode::Char('c') => {
                        // TODO: collapse/expand selected agent
                    }
                    _ => {}
                }
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    std::io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
```

- [ ] **Step 1b: Add dependencies to Cargo.toml**

Add `anyhow = "1"` and `dirs = "6"` to `[dependencies]` in Cargo.toml.

- [ ] **Step 2: Create stub cli module**

Create `src/cli/mod.rs`:

```rust
pub fn init() -> anyhow::Result<()> {
    println!("TODO: init");
    Ok(())
}

pub fn clean() -> anyhow::Result<()> {
    println!("TODO: clean");
    Ok(())
}
```

- [ ] **Step 3: Verify it builds**

Run: `cargo build`
Expected: compiles with no errors

- [ ] **Step 4: Manual smoke test**

Run: `cargo run`
Expected: TUI launches showing "Waiting for events...", press `q` to quit, terminal restores cleanly.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/cli/ Cargo.toml
git commit -m "feat: integrate TUI main loop with socket listener"
```

---

### Task 8: CLI — Hook Installation (`cli/mod.rs`)

**Files:**
- Modify: `src/cli/mod.rs`

- [ ] **Step 1: Write tests for hook merging logic**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_hooks_into_empty_settings() {
        let settings = r#"{}"#;
        let result = merge_hooks(settings).unwrap();
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        let hooks = v.get("hooks").unwrap();
        assert!(hooks.get("PreToolUse").unwrap().as_array().unwrap().len() == 1);
        assert!(hooks.get("SubagentStart").unwrap().as_array().unwrap().len() == 1);
    }

    #[test]
    fn test_merge_hooks_preserves_existing() {
        let settings = r#"{
            "hooks": {
                "PreToolUse": [{ "command": "echo existing" }]
            }
        }"#;
        let result = merge_hooks(settings).unwrap();
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        let pre = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2); // existing + goggles
        assert_eq!(pre[0]["command"].as_str().unwrap(), "echo existing");
    }

    #[test]
    fn test_merge_hooks_idempotent() {
        let settings = r#"{}"#;
        let first = merge_hooks(settings).unwrap();
        let second = merge_hooks(&first).unwrap();
        let v: serde_json::Value = serde_json::from_str(&second).unwrap();
        // Should not duplicate
        assert_eq!(v["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_remove_hooks() {
        let settings = r#"{
            "hooks": {
                "PreToolUse": [
                    { "command": "echo existing" },
                    { "command": "cat | nc -U ~/.claude-goggles/goggles.sock 2>/dev/null || true" }
                ]
            }
        }"#;
        let result = remove_hooks(settings).unwrap();
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        let pre = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 1);
        assert_eq!(pre[0]["command"].as_str().unwrap(), "echo existing");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test cli`
Expected: FAIL — `merge_hooks` and `remove_hooks` not defined

- [ ] **Step 3: Implement merge_hooks, remove_hooks, init, and clean**

```rust
use std::fs;
use std::path::PathBuf;

const GOGGLES_MARKER: &str = "claude-goggles/goggles.sock";

const HOOK_COMMAND: &str = "cat | nc -U ~/.claude-goggles/goggles.sock 2>/dev/null || true";

const HOOK_TYPES: &[&str] = &[
    "PreToolUse",
    "PostToolUse",
    "SubagentStart",
    "SubagentStop",
    "Stop",
];

fn settings_path() -> PathBuf {
    dirs::home_dir()
        .expect("no home dir")
        .join(".claude")
        .join("settings.json")
}

fn socket_dir() -> PathBuf {
    dirs::home_dir()
        .expect("no home dir")
        .join(".claude-goggles")
}

pub fn init() -> anyhow::Result<()> {
    // Ensure socket dir exists
    fs::create_dir_all(socket_dir())?;

    // Read or create settings
    let path = settings_path();
    let content = if path.exists() {
        fs::read_to_string(&path)?
    } else {
        "{}".to_string()
    };

    let updated = merge_hooks(&content)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, &updated)?;
    println!("Hooks installed into {}", path.display());
    println!("Socket dir: {}", socket_dir().display());
    Ok(())
}

pub fn clean() -> anyhow::Result<()> {
    let path = settings_path();
    if path.exists() {
        let content = fs::read_to_string(&path)?;
        let updated = remove_hooks(&content)?;
        fs::write(&path, &updated)?;
        println!("Hooks removed from {}", path.display());
    }

    let sock = socket_dir().join("goggles.sock");
    if sock.exists() {
        fs::remove_file(&sock)?;
        println!("Socket removed");
    }
    Ok(())
}

fn merge_hooks(settings_json: &str) -> anyhow::Result<String> {
    let mut v: serde_json::Value = serde_json::from_str(settings_json)?;

    let hooks = v
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    for hook_type in HOOK_TYPES {
        let arr = hooks
            .as_object_mut()
            .unwrap()
            .entry(*hook_type)
            .or_insert_with(|| serde_json::json!([]));

        let entries = arr.as_array_mut().unwrap();

        // Check if already installed
        let already = entries.iter().any(|e| {
            e.get("command")
                .and_then(|c| c.as_str())
                .is_some_and(|s| s.contains(GOGGLES_MARKER))
        });

        if !already {
            entries.push(serde_json::json!({ "command": HOOK_COMMAND }));
        }
    }

    Ok(serde_json::to_string_pretty(&v)?)
}

fn remove_hooks(settings_json: &str) -> anyhow::Result<String> {
    let mut v: serde_json::Value = serde_json::from_str(settings_json)?;

    if let Some(hooks) = v.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        for hook_type in HOOK_TYPES {
            if let Some(arr) = hooks.get_mut(*hook_type).and_then(|a| a.as_array_mut()) {
                arr.retain(|e| {
                    !e.get("command")
                        .and_then(|c| c.as_str())
                        .is_some_and(|s| s.contains(GOGGLES_MARKER))
                });
            }
        }
    }

    Ok(serde_json::to_string_pretty(&v)?)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test cli`
Expected: all 4 CLI tests pass

- [ ] **Step 5: Commit**

```bash
git add src/cli/
git commit -m "feat: add hook installation and removal CLI commands"
```

---

### Task 9: End-to-End Smoke Test

**Files:**
- No new files

- [ ] **Step 1: Build release binary**

Run: `cargo build --release`
Expected: compiles with no errors

- [ ] **Step 2: Run all tests**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 3: Manual end-to-end test**

Open two terminal panes:

**Pane 1:** Launch the TUI
```bash
cargo run --release
```

**Pane 2:** Send a synthetic event to the socket
```bash
echo '{"session_id":"test-1","hook_event_name":"PreToolUse","tool_name":"Read","tool_input":{"file_path":"src/main.rs"},"tool_use_id":"tu-1"}' | nc -U ~/.claude-goggles/goggles.sock
```

**Expected in Pane 1:** The TUI shows a root agent with status `Running: Read src/main.rs`.

Send a SubagentStart:
```bash
echo '{"session_id":"test-1","hook_event_name":"SubagentStart","agent_id":"agent-1","agent_type":"Explore"}' | nc -U ~/.claude-goggles/goggles.sock
```

**Expected:** A child agent appears in the tree under root.

Press `q` to quit. Terminal restores cleanly.

- [ ] **Step 4: Commit any fixes from smoke test**

```bash
git add -A
git commit -m "fix: address issues found in end-to-end smoke test"
```

(Skip this commit if no fixes were needed.)

---

### Task 10: Final Cleanup + CLAUDE.md

**Files:**
- Create: `CLAUDE.md`

- [ ] **Step 1: Create CLAUDE.md**

```markdown
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
```

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: add CLAUDE.md with build commands and architecture overview"
```
