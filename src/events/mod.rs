use crate::model::TokenUsage;
use serde_json::Value;

pub mod socket;
pub mod transcript;

/// A parsed hook event from Claude Code's event system.
///
/// Each variant corresponds to a lifecycle event emitted by Claude Code hooks
/// over the Unix Domain Socket.
#[derive(Debug)]
pub enum HookEvent {
    PreToolUse {
        session_id: String,
        agent_id: Option<String>,
        tool_name: String,
        key_arg: String,
        tool_use_id: String,
        /// If this is an Agent tool call, the description
        spawns_agent: Option<String>,
    },
    PostToolUse {
        session_id: String,
        agent_id: Option<String>,
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
        transcript_path: Option<String>,
    },
    Stop {
        session_id: String,
        token_usage: Option<TokenUsage>,
        transcript_path: Option<String>,
    },
}

/// Parse a JSON string from a hook event into a typed `HookEvent`.
///
/// Returns `None` if the JSON is malformed or represents an unknown event type.
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
                Some(desc)
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
            let agent_id = v.get("agent_id").and_then(|v| v.as_str()).map(String::from);
            Some(HookEvent::PostToolUse {
                session_id,
                agent_id,
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
            let transcript_path = v.get("agent_transcript_path").and_then(|v| v.as_str()).map(String::from);
            Some(HookEvent::SubagentStop {
                session_id,
                agent_id,
                agent_type,
                token_usage: None,
                transcript_path,
            })
        }
        "Stop" => {
            let transcript_path = v.get("transcript_path").and_then(|v| v.as_str()).map(String::from);
            Some(HookEvent::Stop {
                session_id,
                token_usage: None,
                transcript_path,
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
                assert_eq!(spawns_agent.unwrap(), "Explore codebase");
                assert_eq!(key_arg, "Explore codebase");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_parse_bash_truncates_command() {
        let long_cmd = "a".repeat(100);
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

    #[test]
    fn test_parse_post_tool_use() {
        let json = r#"{
            "session_id": "sess-42",
            "hook_event_name": "PostToolUse",
            "tool_name": "Read",
            "tool_input": { "file_path": "/etc/hosts" },
            "tool_use_id": "tu-99",
            "agent_id": "agent-7"
        }"#;
        let event = parse_hook_event(json).unwrap();
        match event {
            HookEvent::PostToolUse { agent_id, session_id } => {
                assert_eq!(agent_id, Some("agent-7".to_string()));
                assert_eq!(session_id, "sess-42");
            }
            _ => panic!("expected PostToolUse variant"),
        }
    }

    #[test]
    fn test_parse_subagent_start() {
        let json = r#"{
            "session_id": "sess-10",
            "hook_event_name": "SubagentStart",
            "agent_id": "agent-abc",
            "agent_type": "CodeReview"
        }"#;
        let event = parse_hook_event(json).unwrap();
        match event {
            HookEvent::SubagentStart { session_id, agent_id, agent_type } => {
                assert_eq!(session_id, "sess-10");
                assert_eq!(agent_id, "agent-abc");
                assert_eq!(agent_type, "CodeReview");
            }
            _ => panic!("expected SubagentStart variant"),
        }
    }

    #[test]
    fn test_parse_stop_no_transcript() {
        let json = r#"{
            "session_id": "sess-end",
            "hook_event_name": "Stop"
        }"#;
        let event = parse_hook_event(json).unwrap();
        match event {
            HookEvent::Stop { session_id, token_usage, .. } => {
                assert_eq!(session_id, "sess-end");
                assert!(token_usage.is_none());
            }
            _ => panic!("expected Stop variant"),
        }
    }

    #[test]
    fn test_extract_key_arg_write() {
        let input: Value = serde_json::json!({ "file_path": "/tmp/out.txt" });
        assert_eq!(extract_key_arg("Write", &input), "/tmp/out.txt");
    }

    #[test]
    fn test_extract_key_arg_edit() {
        let input: Value = serde_json::json!({ "file_path": "/src/lib.rs" });
        assert_eq!(extract_key_arg("Edit", &input), "/src/lib.rs");
    }

    #[test]
    fn test_extract_key_arg_grep() {
        let input: Value = serde_json::json!({ "pattern": "TODO.*fix" });
        assert_eq!(extract_key_arg("Grep", &input), "TODO.*fix");
    }

    #[test]
    fn test_extract_key_arg_glob() {
        let input: Value = serde_json::json!({ "pattern": "**/*.rs" });
        assert_eq!(extract_key_arg("Glob", &input), "**/*.rs");
    }

    #[test]
    fn test_extract_key_arg_unknown_tool() {
        let input: Value = serde_json::json!({ "some_field": "value" });
        assert_eq!(extract_key_arg("CustomTool", &input), "CustomTool");
    }

    #[test]
    fn test_parse_subagent_stop_field_values() {
        let json = r#"{
            "session_id": "sess-1",
            "hook_event_name": "SubagentStop",
            "agent_id": "agent-1",
            "agent_type": "Explore"
        }"#;
        let event = parse_hook_event(json).unwrap();
        match event {
            HookEvent::SubagentStop { session_id, agent_id, agent_type, token_usage, .. } => {
                assert_eq!(session_id, "sess-1");
                assert_eq!(agent_id, "agent-1");
                assert_eq!(agent_type, "Explore");
                assert!(token_usage.is_none());
            }
            _ => panic!("expected SubagentStop variant"),
        }
    }

    #[test]
    fn test_parse_pre_tool_use_with_agent_id() {
        let json = r#"{
            "session_id": "sess-5",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": { "command": "ls" },
            "tool_use_id": "tu-55",
            "agent_id": "agent-sub-3"
        }"#;
        let event = parse_hook_event(json).unwrap();
        match event {
            HookEvent::PreToolUse { agent_id, session_id, tool_name, .. } => {
                assert_eq!(agent_id, Some("agent-sub-3".to_string()));
                assert_eq!(session_id, "sess-5");
                assert_eq!(tool_name, "Bash");
            }
            _ => panic!("expected PreToolUse variant"),
        }
    }

    #[test]
    fn test_missing_session_id_returns_none() {
        let json = r#"{
            "hook_event_name": "PreToolUse",
            "tool_name": "Read",
            "tool_input": { "file_path": "/tmp/x" },
            "tool_use_id": "tu-1"
        }"#;
        assert!(parse_hook_event(json).is_none());
    }
}
