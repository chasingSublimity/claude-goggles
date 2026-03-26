use std::time::Duration;
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;
use tokio::sync::mpsc;

mod cli;
mod events;
mod model;
mod render;

use events::socket::SocketListener;
use model::AgentTree;
use model::update::apply_event;
use render::Renderer;
use render::tree_view::TreeViewRenderer;

#[derive(Parser)]
#[command(name = "claude-goggles", about = "Visualize Claude Code agent activity")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    /// Visualization mode: tree or bloom
    #[arg(long, default_value = "tree", value_enum)]
    viz: VizMode,
}

#[derive(Subcommand)]
enum Commands {
    /// Install hooks into ~/.claude/settings.json
    Init,
    /// Remove hooks and clean up socket
    Clean,
}

#[derive(Clone, Copy, PartialEq, clap::ValueEnum)]
enum VizMode {
    Tree,
    Bloom,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Init) => cli::init()?,
        Some(Commands::Clean) => cli::clean()?,
        None => {
            run_tui(cli.viz)?;
        }
    }
    Ok(())
}

fn run_tui(initial_mode: VizMode) -> anyhow::Result<()> {
    let sock_path = cli::socket_dir()?.join("goggles.sock");

    let rt = tokio::runtime::Runtime::new()?;
    let (tx, mut rx) = mpsc::channel(1000);

    let listener = SocketListener::new(sock_path);
    rt.spawn(async move {
        if let Err(e) = listener.listen(tx).await {
            eprintln!("Socket error: {}", e);
        }
    });

    enable_raw_mode()?;
    std::io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut tree_renderer = TreeViewRenderer;
    let mut bloom_renderer: Option<render::bloom::BloomRenderer> = match initial_mode {
        VizMode::Bloom => Some(render::bloom::BloomRenderer::new()),
        VizMode::Tree => None,
    };
    let mut viz_mode = initial_mode;
    let mut tree = AgentTree::new();
    let mut scroll_offset: usize = 0;
    let mut selected: usize = 0;

    loop {
        while let Ok(ev) = rx.try_recv() {
            apply_event(&mut tree, ev);
        }

        // Clamp selection to valid range after tree changes
        let visible_count = tree.visible_agent_count();
        if visible_count > 0 {
            selected = selected.min(visible_count - 1);
        } else {
            selected = 0;
        }

        terminal.draw(|frame| {
            match viz_mode {
                VizMode::Tree => tree_renderer.render(&tree, frame, scroll_offset, selected),
                VizMode::Bloom => bloom_renderer
                    .get_or_insert_with(render::bloom::BloomRenderer::new)
                    .render(&tree, frame, scroll_offset, selected),
            }
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Char('v') => {
                        viz_mode = match viz_mode {
                            VizMode::Tree => VizMode::Bloom,
                            VizMode::Bloom => VizMode::Tree,
                        };
                    }
                    _ if viz_mode == VizMode::Tree => match key.code {
                        KeyCode::Up | KeyCode::Char('k') => {
                            selected = selected.saturating_sub(1);
                            let selected_line = selected * 2 + 1;
                            if selected_line < scroll_offset {
                                scroll_offset = selected_line;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if visible_count > 0 {
                                selected = (selected + 1).min(visible_count - 1);
                            }
                            let selected_line = selected * 2 + 1;
                            let area_height = terminal.size()?.height.saturating_sub(2) as usize;
                            if selected_line + 2 > scroll_offset + area_height {
                                scroll_offset = (selected_line + 2).saturating_sub(area_height);
                            }
                        }
                        KeyCode::Char('c') => {
                            if let Some(agent) = tree.nth_visible_agent_mut(selected) {
                                agent.collapsed = !agent.collapsed;
                            }
                        }
                        _ => {}
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    std::io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
