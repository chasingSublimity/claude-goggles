use std::collections::VecDeque;
use std::time::Instant;

/// Input and output token counts for an agent's session.
#[derive(Debug, Clone)]
pub(crate) struct TokenUsage {
    pub input: u64,
    pub output: u64,
}

/// The current activity state of an agent.
#[derive(Debug, Clone)]
pub(crate) enum AgentStatus {
    Idle,
    Running { tool_name: String, key_arg: String },
    Completed,
}

/// A single agent node in the agent tree, with its status, timing, and children.
#[derive(Debug, Clone)]
pub(crate) struct Agent {
    pub id: String,
    pub task: String,
    pub status: AgentStatus,
    pub started_at: Instant,
    pub finished_at: Option<Instant>,
    pub token_usage: Option<TokenUsage>,
    pub children: Vec<Agent>,
    pub collapsed: bool,
}

impl Agent {
    pub(crate) fn new(id: String, task: String) -> Self {
        Self {
            id,
            task,
            status: AgentStatus::Idle,
            started_at: Instant::now(),
            finished_at: None,
            token_usage: None,
            children: Vec::new(),
            collapsed: false,
        }
    }

    /// Find a mutable reference to an agent by ID, searching recursively.
    pub(crate) fn find_mut(&mut self, id: &str) -> Option<&mut Agent> {
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

    /// Collect all agents in the tree as a flat list (depth-first).
    pub(crate) fn all_agents(&self) -> Vec<&Agent> {
        let mut result = vec![self];
        for child in &self.children {
            result.extend(child.all_agents());
        }
        result
    }
}

/// The top-level data structure tracking an entire Claude Code session's agent hierarchy.
#[derive(Debug)]
pub(crate) struct AgentTree {
    pub session_id: Option<String>,
    pub root: Option<Agent>,
    pub dropped_events: u64,
    /// FIFO queue of (parent_agent_id, tool_use_id, description) for pending Agent spawns
    pub pending_spawns: VecDeque<(String, String, String)>,
}

impl AgentTree {
    pub(crate) fn new() -> Self {
        Self {
            session_id: None,
            root: None,
            dropped_events: 0,
            pending_spawns: VecDeque::new(),
        }
    }

    pub(crate) fn find_agent_mut(&mut self, agent_id: Option<&str>) -> Option<&mut Agent> {
        let root = self.root.as_mut()?;
        match agent_id {
            None => Some(root),
            Some(id) => root.find_mut(id),
        }
    }

    /// Return a mutable reference to the nth visible agent (depth-first order).
    /// Respects collapsed state — children of collapsed agents are skipped.
    pub(crate) fn nth_visible_agent_mut(&mut self, n: usize) -> Option<&mut Agent> {
        let root = self.root.as_mut()?;
        let mut counter = 0;
        nth_visible_recursive(root, n, &mut counter)
    }

    /// Count the number of visible agents (respecting collapsed state).
    pub(crate) fn visible_agent_count(&self) -> usize {
        match &self.root {
            None => 0,
            Some(root) => count_visible(root),
        }
    }
}

fn nth_visible_recursive<'a>(agent: &'a mut Agent, target: usize, counter: &mut usize) -> Option<&'a mut Agent> {
    if *counter == target {
        return Some(agent);
    }
    *counter += 1;
    if !agent.collapsed {
        for child in &mut agent.children {
            if let Some(found) = nth_visible_recursive(child, target, counter) {
                return Some(found);
            }
        }
    }
    None
}

fn count_visible(agent: &Agent) -> usize {
    let mut count = 1;
    if !agent.collapsed {
        for child in &agent.children {
            count += count_visible(child);
        }
    }
    count
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

    #[test]
    fn test_find_mut_depth_3() {
        let mut root = Agent::new("root".into(), "Main".into());
        let mut child = Agent::new("child".into(), "Level 1".into());
        let grandchild = Agent::new("grandchild".into(), "Level 2".into());
        child.children.push(grandchild);
        root.children.push(child);

        let found = root.find_mut("grandchild");
        assert!(found.is_some());
        assert_eq!(found.unwrap().task, "Level 2");
    }

    #[test]
    fn test_find_agent_mut_some_id() {
        let mut tree = AgentTree::new();
        let mut root = Agent::new("root".into(), "Main".into());
        root.children.push(Agent::new("child-1".into(), "Sub task".into()));
        tree.root = Some(root);

        let found = tree.find_agent_mut(Some("child-1"));
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "child-1");
    }

    #[test]
    fn test_find_agent_mut_root_is_none() {
        let mut tree = AgentTree::new();
        assert!(tree.find_agent_mut(None).is_none());
        assert!(tree.find_agent_mut(Some("anything")).is_none());
    }

    #[test]
    fn test_visible_agent_count_with_collapsed() {
        let mut tree = AgentTree::new();
        let mut root = Agent::new("root".into(), "Main".into());

        let mut child1 = Agent::new("c1".into(), "Task 1".into());
        child1.children.push(Agent::new("c1a".into(), "Subtask 1a".into()));
        child1.children.push(Agent::new("c1b".into(), "Subtask 1b".into()));
        child1.collapsed = true; // collapse child1, hiding c1a and c1b

        let child2 = Agent::new("c2".into(), "Task 2".into());

        root.children.push(child1);
        root.children.push(child2);
        tree.root = Some(root);

        // root + c1 (collapsed, so c1a/c1b hidden) + c2 = 3
        assert_eq!(tree.visible_agent_count(), 3);
    }

    #[test]
    fn test_visible_agent_count_all_expanded() {
        let mut tree = AgentTree::new();
        let mut root = Agent::new("root".into(), "Main".into());
        let mut child = Agent::new("c1".into(), "Task 1".into());
        child.children.push(Agent::new("c1a".into(), "Subtask".into()));
        root.children.push(child);
        tree.root = Some(root);

        // root + c1 + c1a = 3
        assert_eq!(tree.visible_agent_count(), 3);
    }

    #[test]
    fn test_nth_visible_agent_mut_basic() {
        let mut tree = AgentTree::new();
        let mut root = Agent::new("root".into(), "Main".into());
        root.children.push(Agent::new("c1".into(), "Task 1".into()));
        root.children.push(Agent::new("c2".into(), "Task 2".into()));
        tree.root = Some(root);

        // Index 0 = root, 1 = c1, 2 = c2
        assert_eq!(tree.nth_visible_agent_mut(0).unwrap().id, "root");
        assert_eq!(tree.nth_visible_agent_mut(1).unwrap().id, "c1");
        assert_eq!(tree.nth_visible_agent_mut(2).unwrap().id, "c2");
        assert!(tree.nth_visible_agent_mut(3).is_none());
    }

    #[test]
    fn test_nth_visible_agent_mut_with_collapsed() {
        let mut tree = AgentTree::new();
        let mut root = Agent::new("root".into(), "Main".into());

        let mut child1 = Agent::new("c1".into(), "Task 1".into());
        child1.children.push(Agent::new("c1a".into(), "Subtask 1a".into()));
        child1.children.push(Agent::new("c1b".into(), "Subtask 1b".into()));
        child1.collapsed = true; // collapse: c1a and c1b should be skipped

        let child2 = Agent::new("c2".into(), "Task 2".into());

        root.children.push(child1);
        root.children.push(child2);
        tree.root = Some(root);

        // Index 0 = root, 1 = c1 (collapsed), 2 = c2 (c1a/c1b skipped)
        assert_eq!(tree.nth_visible_agent_mut(0).unwrap().id, "root");
        assert_eq!(tree.nth_visible_agent_mut(1).unwrap().id, "c1");
        assert_eq!(tree.nth_visible_agent_mut(2).unwrap().id, "c2");
        assert!(tree.nth_visible_agent_mut(3).is_none());
    }

    #[test]
    fn test_all_agents_flat_traversal() {
        let mut root = Agent::new("root".into(), "Main".into());
        let mut child = Agent::new("c1".into(), "Task 1".into());
        child.children.push(Agent::new("c1a".into(), "Subtask".into()));
        root.children.push(child);
        root.children.push(Agent::new("c2".into(), "Task 2".into()));

        let all = root.all_agents();
        let ids: Vec<&str> = all.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["root", "c1", "c1a", "c2"]);
    }
}
