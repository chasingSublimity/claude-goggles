use crate::model::TokenUsage;
use serde_json::Value;

pub mod socket;
pub mod transcript;

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
            let transcript_path = v.get("agent_transcript_path").and_then(|v| v.as_str()).map(String::from);
            let token_usage = transcript_path
                .as_deref()
                .and_then(|p| transcript::parse_transcript_usage(std::path::Path::new(p)));
            Some(HookEvent::SubagentStop {
                session_id,
                agent_id,
                agent_type,
                token_usage,
            })
        }
        "Stop" => {
            let transcript_path = v.get("transcript_path").and_then(|v| v.as_str()).map(String::from);
            let token_usage = transcript_path
                .as_deref()
                .and_then(|p| transcript::parse_transcript_usage(std::path::Path::new(p)));
            Some(HookEvent::Stop {
                session_id,
                token_usage,
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
                assert_eq!(spawns_agent.unwrap().0, "Explore codebase");
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
}
