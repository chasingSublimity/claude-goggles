use crate::model::{Agent, AgentStatus, AgentTree};

/// Format a token count for display (e.g., "500 tok" or "3.1k tok").
pub(crate) fn format_tokens(total: u64) -> String {
    if total >= 1000 {
        format!("{:.1}k tok", total as f64 / 1000.0)
    } else {
        format!("{} tok", total)
    }
}

/// Count (active, total) agents in the tree.
pub(crate) fn count_agents(tree: &AgentTree) -> (usize, usize) {
    match &tree.root {
        None => (0, 0),
        Some(root) => {
            let all = root.all_agents();
            let total = all.len();
            let active = all
                .iter()
                .filter(|a| !matches!(a.status, AgentStatus::Completed))
                .count();
            (active, total)
        }
    }
}

/// Sum all token usage across the tree.
pub(crate) fn sum_tokens(tree: &AgentTree) -> u64 {
    match &tree.root {
        None => 0,
        Some(root) => sum_tokens_recursive(root),
    }
}

fn sum_tokens_recursive(agent: &Agent) -> u64 {
    let own = agent
        .token_usage
        .as_ref()
        .map_or(0, |t| t.input + t.output);
    own + agent.children.iter().map(sum_tokens_recursive).sum::<u64>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TokenUsage;

    #[test]
    fn test_format_tokens_zero() {
        assert_eq!(format_tokens(0), "0 tok");
    }

    #[test]
    fn test_format_tokens_small() {
        assert_eq!(format_tokens(500), "500 tok");
    }

    #[test]
    fn test_format_tokens_999() {
        assert_eq!(format_tokens(999), "999 tok");
    }

    #[test]
    fn test_format_tokens_1000() {
        assert_eq!(format_tokens(1000), "1.0k tok");
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
        assert_eq!(count_agents(&tree), (2, 3));
    }

    #[test]
    fn test_count_agents_completed_not_active() {
        let mut tree = AgentTree::new();
        let mut root = Agent::new("root".into(), "Main".into());
        root.status = AgentStatus::Completed;
        let mut c1 = Agent::new("c1".into(), "Task 1".into());
        c1.status = AgentStatus::Completed;
        root.children.push(c1);
        root.children.push(Agent::new("c2".into(), "Task 2".into()));
        tree.root = Some(root);
        let (active, total) = count_agents(&tree);
        assert_eq!(total, 3);
        assert_eq!(active, 1);
    }

    #[test]
    fn test_sum_tokens_empty_tree() {
        let tree = AgentTree::new();
        assert_eq!(sum_tokens(&tree), 0);
    }

    #[test]
    fn test_sum_tokens_with_usage() {
        let mut tree = AgentTree::new();
        let mut root = Agent::new("root".into(), "Main".into());
        root.token_usage = Some(TokenUsage { input: 100, output: 50 });
        let mut child = Agent::new("c1".into(), "Sub".into());
        child.token_usage = Some(TokenUsage { input: 200, output: 300 });
        root.children.push(child);
        root.children.push(Agent::new("c2".into(), "Sub2".into()));
        tree.root = Some(root);
        assert_eq!(sum_tokens(&tree), 650);
    }
}
