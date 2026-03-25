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
