# Repository Quality Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Address README gaps, Rust best practices issues, and test coverage gaps identified in the repository audit.

**Architecture:** Changes span README docs, Cargo.toml config, visibility modifiers across all modules, doc comments on public types, and new test functions in three modules. No new files are created (except this plan). All changes are additive or corrective -- no architectural changes.

**Tech Stack:** Rust, ratatui (TestBackend for render tests), tokio (for async tests), tempfile (dev-dependency)

---

### Task 1: Fix README gaps

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add missing keybindings and fix fps claim**

Update the keyboard controls table to include `v` and bloom mode controls, and fix the "~10fps" claim:

```markdown
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
```

Also change "re-renders at ~10fps" to "re-renders at ~60fps" in the "How it works" section.

- [ ] **Step 2: Verify README reads correctly**

Run: `head -50 README.md`
Expected: Updated keyboard table with all keys, corrected fps claim.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: add missing keybindings and fix fps claim in README"
```

---

### Task 2: Narrow tokio features

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Replace tokio "full" with specific features**

Change:
```toml
tokio = { version = "1", features = ["full"] }
```
To:
```toml
tokio = { version = "1", features = ["rt-multi-thread", "net", "sync", "macros", "io-util", "time"] }
```

These cover the actual usage:
- `rt-multi-thread`: `Runtime::new()` in main.rs
- `net`: `UnixListener` in socket.rs
- `sync`: `mpsc` channel
- `macros`: `#[tokio::test]`
- `io-util`: `AsyncReadExt` in socket.rs
- `time`: `tokio::time::sleep` in tests

- [ ] **Step 2: Verify it builds and tests pass**

Run: `cargo test 2>&1 | tail -5`
Expected: `test result: ok. 100 passed`

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "build: narrow tokio features from full to specific needs"
```

---

### Task 3: Add doc comments to public types

**Files:**
- Modify: `src/events/mod.rs`
- Modify: `src/model/mod.rs`
- Modify: `src/render/mod.rs`
- Modify: `src/render/bloom.rs`

- [ ] **Step 1: Add doc comments to events/mod.rs**

Add above `pub enum HookEvent`:
```rust
/// A parsed hook event from Claude Code's event system.
///
/// Each variant corresponds to a lifecycle event emitted by Claude Code hooks
/// over the Unix Domain Socket.
```

Add above `pub fn parse_hook_event`:
```rust
/// Parse a JSON string from a hook event into a typed `HookEvent`.
///
/// Returns `None` if the JSON is malformed or represents an unknown event type.
```

- [ ] **Step 2: Add doc comments to model/mod.rs**

Add above `pub struct TokenUsage`:
```rust
/// Input and output token counts for an agent's session.
```

Add above `pub enum AgentStatus`:
```rust
/// The current activity state of an agent.
```

Add above `pub struct Agent`:
```rust
/// A single agent node in the agent tree, with its status, timing, and children.
```

Add above `pub struct AgentTree`:
```rust
/// The top-level data structure tracking an entire Claude Code session's agent hierarchy.
```

- [ ] **Step 3: Add doc comments to render types**

In `src/render/mod.rs`, add above `pub trait Renderer`:
```rust
/// Trait for visualization backends that render an `AgentTree` to a terminal frame.
```

In `src/render/bloom.rs`, add above `pub struct BloomParams`:
```rust
/// Tunable parameters for the bloom visualization (sphere sizes, bloom spread, physics).
```

- [ ] **Step 4: Verify it builds**

Run: `cargo build 2>&1 | tail -3`
Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add src/events/mod.rs src/model/mod.rs src/render/mod.rs src/render/bloom.rs
git commit -m "docs: add doc comments to public types and key functions"
```

---

### Task 4: Use pub(crate) for internal items

**Files:**
- Modify: `src/events/mod.rs`
- Modify: `src/events/socket.rs`
- Modify: `src/events/transcript.rs`
- Modify: `src/render/mod.rs`
- Modify: `src/render/bloom.rs`
- Modify: `src/render/tree_view.rs`
- Modify: `src/render/footer.rs`
- Modify: `src/model/mod.rs`
- Modify: `src/cli/mod.rs`

- [ ] **Step 1: Change pub to pub(crate) across all modules**

Since this is a binary crate with no lib.rs, all `pub` items are internal. Change `pub` to `pub(crate)` for:

In `src/events/mod.rs`:
- `pub enum HookEvent` -> `pub(crate) enum HookEvent`
- `pub fn parse_hook_event` -> `pub(crate) fn parse_hook_event`

In `src/events/socket.rs`:
- `pub struct SocketListener` -> `pub(crate) struct SocketListener`

In `src/events/transcript.rs`:
- `pub fn parse_transcript_usage` -> `pub(crate) fn parse_transcript_usage`

In `src/model/mod.rs`:
- `pub struct TokenUsage` -> `pub(crate) struct TokenUsage`
- `pub enum AgentStatus` -> `pub(crate) enum AgentStatus`
- `pub struct Agent` -> `pub(crate) struct Agent`
- `pub struct AgentTree` -> `pub(crate) struct AgentTree`
- All `pub fn` methods on Agent and AgentTree -> `pub(crate) fn`
- All `pub` fields stay `pub` (needed for struct literal construction within crate)

In `src/render/mod.rs`:
- `pub trait Renderer` -> `pub(crate) trait Renderer`

In `src/render/tree_view.rs`:
- `pub struct TreeViewRenderer` -> `pub(crate) struct TreeViewRenderer`

In `src/render/bloom.rs`:
- `pub struct BloomParams` -> `pub(crate) struct BloomParams`
- `pub struct BloomRenderer` -> `pub(crate) struct BloomRenderer`
- `pub fn new()` on BloomRenderer -> `pub(crate) fn new()`
- All `pub fn` methods on BloomParams -> `pub(crate) fn`

In `src/render/footer.rs`:
- `pub fn format_tokens` -> `pub(crate) fn format_tokens`
- `pub fn count_agents` -> `pub(crate) fn count_agents`
- `pub fn sum_tokens` -> `pub(crate) fn sum_tokens`

In `src/cli/mod.rs`:
- `pub fn socket_dir` -> `pub(crate) fn socket_dir`
- `pub fn init` -> `pub(crate) fn init`
- `pub fn clean` -> `pub(crate) fn clean`

- [ ] **Step 2: Verify it builds and tests pass**

Run: `cargo test 2>&1 | tail -5`
Expected: `test result: ok. 100 passed`

- [ ] **Step 3: Commit**

```bash
git add src/
git commit -m "refactor: use pub(crate) for internal items in binary crate"
```

---

### Task 5: Add tree_view render tests

**Files:**
- Modify: `src/render/tree_view.rs`

- [ ] **Step 1: Write test for render_agent output with a simple tree**

Add to the `#[cfg(test)] mod tests` block in `src/render/tree_view.rs`:

```rust
use crate::model::{Agent, AgentStatus, AgentTree, TokenUsage};

fn lines_to_text(lines: &[Line]) -> Vec<String> {
    lines.iter().map(|line| {
        line.spans.iter().map(|s| s.content.as_ref()).collect::<String>()
    }).collect()
}

#[test]
fn test_render_agent_root_idle() {
    let root = Agent::new("root".into(), "Main task".into());
    let mut lines = Vec::new();
    let mut agent_index = 0;
    render_agent(&mut lines, &root, "", true, 999, &mut agent_index);

    let text = lines_to_text(&lines);
    assert_eq!(text.len(), 2, "should produce 2 lines (name + status)");
    assert!(text[0].contains("root"), "first line should contain agent id");
    assert!(text[0].contains("Main task"), "first line should contain task");
    assert!(text[1].contains("idle"), "second line should show idle status");
}

#[test]
fn test_render_agent_running_shows_tool() {
    let mut root = Agent::new("root".into(), "Main".into());
    root.status = AgentStatus::Running {
        tool_name: "Read".into(),
        key_arg: "/src/main.rs".into(),
    };
    let mut lines = Vec::new();
    let mut agent_index = 0;
    render_agent(&mut lines, &root, "", true, 999, &mut agent_index);

    let text = lines_to_text(&lines);
    assert!(text[1].contains("Read"), "status line should show tool name");
    assert!(text[1].contains("/src/main.rs"), "status line should show key arg");
}

#[test]
fn test_render_agent_completed_shows_done() {
    let mut root = Agent::new("root".into(), "Main".into());
    root.status = AgentStatus::Completed;
    let mut lines = Vec::new();
    let mut agent_index = 0;
    render_agent(&mut lines, &root, "", true, 999, &mut agent_index);

    let text = lines_to_text(&lines);
    assert!(text[1].contains("done"), "completed agent should show 'done'");
}

#[test]
fn test_render_agent_with_children() {
    let mut root = Agent::new("root".into(), "Main".into());
    root.children.push(Agent::new("c1".into(), "Sub 1".into()));
    root.children.push(Agent::new("c2".into(), "Sub 2".into()));
    let mut lines = Vec::new();
    let mut agent_index = 0;
    render_agent(&mut lines, &root, "", true, 999, &mut agent_index);

    let text = lines_to_text(&lines);
    // root (2 lines) + c1 (2 lines) + c2 (2 lines) = 6
    assert_eq!(text.len(), 6, "root + 2 children = 6 lines, got {}", text.len());
    assert!(text[2].contains("c1"), "third line should be first child");
    assert!(text[4].contains("c2"), "fifth line should be second child");
}

#[test]
fn test_render_agent_collapsed_hides_children() {
    let mut root = Agent::new("root".into(), "Main".into());
    root.children.push(Agent::new("c1".into(), "Sub 1".into()));
    root.collapsed = true;
    let mut lines = Vec::new();
    let mut agent_index = 0;
    render_agent(&mut lines, &root, "", true, 999, &mut agent_index);

    let text = lines_to_text(&lines);
    assert_eq!(text.len(), 2, "collapsed root should hide children");
    assert!(text[0].contains("[+]"), "collapsed node should show [+] indicator");
}

#[test]
fn test_render_agent_selection_highlight() {
    let root = Agent::new("root".into(), "Main".into());
    let mut lines = Vec::new();
    let mut agent_index = 0;
    // selected=0 means root is selected
    render_agent(&mut lines, &root, "", true, 0, &mut agent_index);

    // Check that the selected line has a background color set
    let first_line = &lines[0];
    let has_bg = first_line.spans.iter().any(|s| {
        matches!(s.style.bg, Some(Color::DarkGray))
    });
    assert!(has_bg, "selected agent should have DarkGray background");
}
```

- [ ] **Step 2: Run the new tests**

Run: `cargo test tree_view 2>&1`
Expected: All new tests pass along with the existing `test_format_duration_pattern`.

- [ ] **Step 3: Commit**

```bash
git add src/render/tree_view.rs
git commit -m "test: add render_agent unit tests for tree_view"
```

---

### Task 6: Add resolve_transcript_usage tests

**Files:**
- Modify: `src/events/socket.rs`

- [ ] **Step 1: Write tests for resolve_transcript_usage**

Add to the `#[cfg(test)] mod tests` block in `src/events/socket.rs`:

```rust
use std::io::Write;

#[tokio::test]
async fn test_resolve_transcript_subagent_stop_with_file() {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    writeln!(f, r#"{{"type":"assistant","message":{{"usage":{{"input_tokens":100,"output_tokens":50}}}}}}"#).unwrap();
    f.flush().unwrap();

    let event = HookEvent::SubagentStop {
        session_id: "s1".into(),
        agent_id: "a1".into(),
        agent_type: "Explore".into(),
        token_usage: None,
        transcript_path: Some(f.path().to_string_lossy().to_string()),
    };

    let resolved = resolve_transcript_usage(event).await;
    match resolved {
        HookEvent::SubagentStop { token_usage, .. } => {
            let usage = token_usage.expect("should have resolved token usage");
            assert_eq!(usage.input, 100);
            assert_eq!(usage.output, 50);
        }
        _ => panic!("expected SubagentStop"),
    }
}

#[tokio::test]
async fn test_resolve_transcript_subagent_stop_no_path() {
    let event = HookEvent::SubagentStop {
        session_id: "s1".into(),
        agent_id: "a1".into(),
        agent_type: "Explore".into(),
        token_usage: None,
        transcript_path: None,
    };

    let resolved = resolve_transcript_usage(event).await;
    match resolved {
        HookEvent::SubagentStop { token_usage, .. } => {
            assert!(token_usage.is_none());
        }
        _ => panic!("expected SubagentStop"),
    }
}

#[tokio::test]
async fn test_resolve_transcript_stop_with_file() {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    writeln!(f, r#"{{"type":"assistant","message":{{"usage":{{"input_tokens":500,"output_tokens":200}}}}}}"#).unwrap();
    f.flush().unwrap();

    let event = HookEvent::Stop {
        session_id: "s1".into(),
        token_usage: None,
        transcript_path: Some(f.path().to_string_lossy().to_string()),
    };

    let resolved = resolve_transcript_usage(event).await;
    match resolved {
        HookEvent::Stop { token_usage, .. } => {
            let usage = token_usage.expect("should have resolved token usage");
            assert_eq!(usage.input, 500);
            assert_eq!(usage.output, 200);
        }
        _ => panic!("expected Stop"),
    }
}

#[tokio::test]
async fn test_resolve_transcript_passthrough_other_events() {
    let event = HookEvent::PreToolUse {
        session_id: "s1".into(),
        agent_id: None,
        tool_name: "Read".into(),
        key_arg: "file.rs".into(),
        tool_use_id: "tu-1".into(),
        spawns_agent: None,
    };

    let resolved = resolve_transcript_usage(event).await;
    assert!(matches!(resolved, HookEvent::PreToolUse { .. }));
}
```

- [ ] **Step 2: Run the new tests**

Run: `cargo test socket 2>&1`
Expected: All 6 socket tests pass (2 existing + 4 new).

- [ ] **Step 3: Commit**

```bash
git add src/events/socket.rs
git commit -m "test: add resolve_transcript_usage tests for socket module"
```

---

### Task 7: Add cli init/clean tests

**Files:**
- Modify: `src/cli/mod.rs`

- [ ] **Step 1: Write tests for remove_hooks edge cases**

Add to the `#[cfg(test)] mod tests` block in `src/cli/mod.rs`:

```rust
#[test]
fn test_remove_hooks_no_hooks_section() {
    let settings = r#"{ "some_other_key": true }"#;
    let result = remove_hooks(settings).unwrap();
    let v: serde_json::Value = serde_json::from_str(&result).unwrap();
    // Should not crash, hooks section stays absent
    assert!(v.get("some_other_key").unwrap().as_bool().unwrap());
}

#[test]
fn test_remove_hooks_empty_hooks() {
    let settings = r#"{ "hooks": {} }"#;
    let result = remove_hooks(settings).unwrap();
    let v: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(v["hooks"].as_object().unwrap().is_empty());
}

#[test]
fn test_remove_hooks_no_goggles_entries() {
    let settings = r#"{
        "hooks": {
            "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "echo hi" }] }]
        }
    }"#;
    let result = remove_hooks(settings).unwrap();
    let v: serde_json::Value = serde_json::from_str(&result).unwrap();
    let pre = v["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(pre.len(), 1, "non-goggles hooks should be preserved");
}

#[test]
fn test_merge_then_remove_roundtrip() {
    let original = r#"{ "hooks": { "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "echo existing" }] }] } }"#;
    let merged = merge_hooks(original).unwrap();
    let cleaned = remove_hooks(&merged).unwrap();
    let v: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
    let pre = v["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(pre.len(), 1);
    assert_eq!(pre[0]["hooks"][0]["command"].as_str().unwrap(), "echo existing");
}
```

- [ ] **Step 2: Run the new tests**

Run: `cargo test cli 2>&1`
Expected: All 8 cli tests pass (4 existing + 4 new).

- [ ] **Step 3: Commit**

```bash
git add src/cli/mod.rs
git commit -m "test: add remove_hooks edge case and roundtrip tests for cli"
```
