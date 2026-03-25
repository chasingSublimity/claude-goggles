use super::{Agent, AgentStatus, AgentTree, ToolCall};
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
    use crate::model::TokenUsage;

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
