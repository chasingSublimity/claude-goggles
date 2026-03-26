use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::time::Instant;
use crate::model::{Agent, AgentStatus, AgentTree};
use super::Renderer;
use super::footer::{format_tokens, count_agents, sum_tokens};

pub struct TreeViewRenderer;

impl Renderer for TreeViewRenderer {
    fn render(&mut self, tree: &AgentTree, frame: &mut Frame, scroll_offset: usize, selected: usize) {
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
            let mut agent_index = 0;
            render_agent(&mut lines, root, "", true, selected, &mut agent_index);
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
        let total_tokens = sum_tokens(tree);
        let footer = Line::from(vec![
            Span::styled(
                format!("agents: {} ({} active)", total, active),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format_tokens(total_tokens),
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

fn render_agent(
    lines: &mut Vec<Line>,
    agent: &Agent,
    prefix: &str,
    is_last: bool,
    selected: usize,
    agent_index: &mut usize,
) {
    let is_selected = *agent_index == selected;
    *agent_index += 1;

    let connector = if prefix.is_empty() { "" } else if is_last { "└─ " } else { "├─ " };
    let status_icon = match &agent.status {
        AgentStatus::Completed => Span::styled("◯ ", Style::default().fg(Color::DarkGray)),
        _ => Span::styled("● ", Style::default().fg(Color::Green)),
    };

    let collapse_indicator = if !agent.children.is_empty() && agent.collapsed {
        " [+]"
    } else {
        ""
    };

    let elapsed = format_duration(agent.started_at);
    let tokens = match &agent.token_usage {
        Some(t) => format_tokens(t.input + t.output),
        None => "—".to_string(),
    };

    let highlight = if is_selected {
        Style::default().bg(Color::DarkGray).fg(Color::White)
    } else {
        Style::default()
    };

    let id_style = if is_selected {
        highlight.fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let task_style = if is_selected {
        highlight
    } else if matches!(agent.status, AgentStatus::Completed) {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
    };

    lines.push(Line::from(vec![
        Span::styled(prefix.to_string(), Style::default().fg(Color::DarkGray)),
        Span::styled(connector.to_string(), Style::default().fg(Color::DarkGray)),
        status_icon,
        Span::styled(format!("{} ", agent.id), id_style),
        Span::styled("─ ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{}{}", agent.task, collapse_indicator), task_style),
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
            render_agent(lines, child, &child_prefix, is_last_child, selected, agent_index);
        }
    }
}

fn format_duration(started: Instant) -> String {
    let secs = started.elapsed().as_secs();
    let mins = secs / 60;
    let secs = secs % 60;
    format!("{}m {:02}s", mins, secs)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_pattern() {
        let started = Instant::now();
        let result = format_duration(started);
        assert!(result.contains('m'), "expected 'm' in duration string: {}", result);
        assert!(result.contains('s'), "expected 's' in duration string: {}", result);
        assert!(result.starts_with("0m"), "expected to start with '0m': {}", result);
    }
}
