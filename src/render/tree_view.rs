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
