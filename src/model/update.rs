use super::{Agent, AgentStatus, AgentTree};
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
            if let Some(desc) = spawns_agent {
                let parent_id = agent_id.clone().unwrap_or_else(|| "root".into());
                tree.pending_spawns.push_back((parent_id, tool_use_id, desc));
            }
            if let Some(agent) = tree.find_agent_mut(agent_id.as_deref()) {
                agent.status = AgentStatus::Running { tool_name, key_arg };
            } else {
                tree.dropped_events += 1;
            }
        }
        HookEvent::PostToolUse {
            session_id,
            agent_id,
            ..
        } => {
            ensure_root(tree, &session_id);
            if let Some(agent) = tree.find_agent_mut(agent_id.as_deref()) {
                agent.status = AgentStatus::Idle;
            } else {
                tree.dropped_events += 1;
            }
        }
        HookEvent::SubagentStart {
            session_id,
            agent_id,
            agent_type,
        } => {
            ensure_root(tree, &session_id);
            // Pop the oldest pending spawn (FIFO order ensures deterministic matching)
            let (parent_id, task_desc) = if let Some((pid, _tuid, desc)) =
                tree.pending_spawns.pop_front()
            {
                (Some(pid), desc)
            } else {
                (None, agent_type)
            };
            let child = Agent::new(agent_id, task_desc);
            if let Some(parent) = tree.find_agent_mut(parent_id.as_deref()) {
                parent.children.push(child);
            } else {
                tree.dropped_events += 1;
            }
        }
        HookEvent::SubagentStop {
            agent_id,
            token_usage,
            ..
        } => {
            let found = if let Some(root) = &mut tree.root {
                if let Some(agent) = root.find_mut(&agent_id) {
                    agent.status = AgentStatus::Completed;
                    agent.finished_at = Some(std::time::Instant::now());
                    agent.token_usage = token_usage;
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if !found {
                tree.dropped_events += 1;
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

    fn make_post_tool_use(agent_id: Option<&str>, _tool: &str, _arg: &str) -> HookEvent {
        HookEvent::PostToolUse {
            session_id: "sess-1".into(),
            agent_id: agent_id.map(String::from),
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
    fn test_post_tool_use_sets_idle() {
        let mut tree = AgentTree::new();
        apply_event(&mut tree, make_pre_tool_use(None, "Read", "src/main.rs"));
        apply_event(&mut tree, make_post_tool_use(None, "Read", "src/main.rs"));

        let root = tree.root.as_ref().unwrap();
        assert!(matches!(root.status, AgentStatus::Idle));
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
                spawns_agent: Some("Explore codebase".into()),
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
                transcript_path: None,
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
                transcript_path: None,
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
                spawns_agent: Some("Write tests".into()),
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
                spawns_agent: Some("Update docs".into()),
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

    // --- Helper to bootstrap a tree with root + one child agent ---
    fn tree_with_subagent() -> AgentTree {
        let mut tree = AgentTree::new();
        // Create root via PreToolUse that spawns an agent
        apply_event(
            &mut tree,
            HookEvent::PreToolUse {
                session_id: "sess-1".into(),
                agent_id: None,
                tool_name: "Agent".into(),
                key_arg: "Explore codebase".into(),
                tool_use_id: "tu-spawn-1".into(),
                spawns_agent: Some("Explore codebase".into()),
            },
        );
        // SubagentStart creates agent-1 as child of root
        apply_event(
            &mut tree,
            HookEvent::SubagentStart {
                session_id: "sess-1".into(),
                agent_id: "agent-1".into(),
                agent_type: "Explore".into(),
            },
        );
        tree
    }

    #[test]
    fn test_pre_tool_use_targeting_subagent() {
        let mut tree = tree_with_subagent();
        // Subagent agent-1 uses a tool
        apply_event(
            &mut tree,
            make_pre_tool_use(Some("agent-1"), "Bash", "cargo test"),
        );

        let root = tree.root.as_ref().unwrap();
        let child = &root.children[0];
        assert_eq!(child.id, "agent-1");
        assert!(matches!(
            &child.status,
            AgentStatus::Running { tool_name, key_arg }
            if tool_name == "Bash" && key_arg == "cargo test"
        ));
        assert_eq!(tree.dropped_events, 0);
    }

    #[test]
    fn test_post_tool_use_targeting_subagent() {
        let mut tree = tree_with_subagent();
        // Subagent uses a tool, then finishes
        apply_event(
            &mut tree,
            make_pre_tool_use(Some("agent-1"), "Read", "lib.rs"),
        );
        apply_event(
            &mut tree,
            make_post_tool_use(Some("agent-1"), "Read", "lib.rs"),
        );

        let root = tree.root.as_ref().unwrap();
        let child = &root.children[0];
        assert!(matches!(child.status, AgentStatus::Idle));
        assert_eq!(tree.dropped_events, 0);
    }

    #[test]
    fn test_pre_tool_use_nonexistent_agent_drops_event() {
        let mut tree = tree_with_subagent();
        assert_eq!(tree.dropped_events, 0);
        apply_event(
            &mut tree,
            make_pre_tool_use(Some("agent-999"), "Read", "file.rs"),
        );
        assert_eq!(tree.dropped_events, 1);
    }

    #[test]
    fn test_post_tool_use_nonexistent_agent_drops_event() {
        let mut tree = tree_with_subagent();
        assert_eq!(tree.dropped_events, 0);
        apply_event(
            &mut tree,
            make_post_tool_use(Some("agent-999"), "Read", "file.rs"),
        );
        assert_eq!(tree.dropped_events, 1);
    }

    #[test]
    fn test_subagent_stop_unknown_agent_drops_event() {
        let mut tree = AgentTree::new();
        // Create a root but no subagents
        apply_event(&mut tree, make_pre_tool_use(None, "Read", "file.rs"));
        assert_eq!(tree.dropped_events, 0);

        apply_event(
            &mut tree,
            HookEvent::SubagentStop {
                session_id: "sess-1".into(),
                agent_id: "agent-unknown".into(),
                agent_type: "Explore".into(),
                token_usage: None,
                transcript_path: None,
            },
        );
        assert_eq!(tree.dropped_events, 1);
    }

    #[test]
    fn test_stop_with_no_root_does_not_panic() {
        let mut tree = AgentTree::new();
        assert!(tree.root.is_none());
        // Stop on empty tree — should not panic
        apply_event(
            &mut tree,
            HookEvent::Stop {
                session_id: "sess-1".into(),
                token_usage: Some(TokenUsage {
                    input: 100,
                    output: 50,
                }),
                transcript_path: None,
            },
        );
        // Root remains None since Stop doesn't call ensure_root
        assert!(tree.root.is_none());
    }

    #[test]
    fn test_ensure_root_idempotency() {
        let mut tree = AgentTree::new();
        // First event creates root
        apply_event(&mut tree, make_pre_tool_use(None, "Read", "a.rs"));
        let root = tree.root.as_ref().unwrap();
        let first_task = root.task.clone();
        let first_id = root.id.clone();

        // Second event should NOT replace the root
        apply_event(&mut tree, make_pre_tool_use(None, "Write", "b.rs"));
        let root = tree.root.as_ref().unwrap();
        assert_eq!(root.id, first_id);
        assert_eq!(root.task, first_task);

        // Third event from a different session_id should still keep original root
        apply_event(
            &mut tree,
            HookEvent::PreToolUse {
                session_id: "sess-other".into(),
                agent_id: None,
                tool_name: "Bash".into(),
                key_arg: "ls".into(),
                tool_use_id: "tu-99".into(),
                spawns_agent: None,
            },
        );
        let root = tree.root.as_ref().unwrap();
        assert_eq!(root.id, first_id);
        assert_eq!(root.task, first_task);
        assert_eq!(tree.session_id.as_deref(), Some("sess-1"));
    }

    #[test]
    fn test_nested_agent_spawns() {
        let mut tree = tree_with_subagent();

        // agent-1 (subagent) itself spawns another agent
        apply_event(
            &mut tree,
            HookEvent::PreToolUse {
                session_id: "sess-1".into(),
                agent_id: Some("agent-1".into()),
                tool_name: "Agent".into(),
                key_arg: "Deep analysis".into(),
                tool_use_id: "tu-spawn-nested".into(),
                spawns_agent: Some("Deep analysis".into()),
            },
        );

        // The grandchild agent starts
        apply_event(
            &mut tree,
            HookEvent::SubagentStart {
                session_id: "sess-1".into(),
                agent_id: "agent-1-1".into(),
                agent_type: "general-purpose".into(),
            },
        );

        // Verify the structure: root -> agent-1 -> agent-1-1
        let root = tree.root.as_ref().unwrap();
        assert_eq!(root.children.len(), 1);

        let child = &root.children[0];
        assert_eq!(child.id, "agent-1");
        assert_eq!(child.children.len(), 1);

        let grandchild = &child.children[0];
        assert_eq!(grandchild.id, "agent-1-1");
        assert_eq!(grandchild.task, "Deep analysis");
        assert!(matches!(grandchild.status, AgentStatus::Idle));

        // Grandchild does some work
        apply_event(
            &mut tree,
            make_pre_tool_use(Some("agent-1-1"), "Grep", "TODO"),
        );

        let grandchild = &tree.root.as_ref().unwrap().children[0].children[0];
        assert!(matches!(
            &grandchild.status,
            AgentStatus::Running { tool_name, .. } if tool_name == "Grep"
        ));

        // Grandchild stops
        apply_event(
            &mut tree,
            HookEvent::SubagentStop {
                session_id: "sess-1".into(),
                agent_id: "agent-1-1".into(),
                agent_type: "general-purpose".into(),
                token_usage: Some(TokenUsage {
                    input: 200,
                    output: 100,
                }),
                transcript_path: None,
            },
        );

        let grandchild = &tree.root.as_ref().unwrap().children[0].children[0];
        assert!(matches!(grandchild.status, AgentStatus::Completed));
        assert_eq!(grandchild.token_usage.as_ref().unwrap().input, 200);

        assert_eq!(tree.dropped_events, 0);
    }
}
