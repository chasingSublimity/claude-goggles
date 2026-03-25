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
}

#[derive(Subcommand)]
enum Commands {
    /// Install hooks into ~/.claude/settings.json
    Init,
    /// Remove hooks and clean up socket
    Clean,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Init) => cli::init()?,
        Some(Commands::Clean) => cli::clean()?,
        None => run_tui()?,
    }
    Ok(())
}

fn run_tui() -> anyhow::Result<()> {
    let sock_path = dirs::home_dir()
        .expect("no home dir")
        .join(".claude-goggles")
        .join("goggles.sock");

    let rt = tokio::runtime::Runtime::new()?;
    let (tx, mut rx) = mpsc::channel(1000);

    // Start socket listener in background
    let listener = SocketListener::new(sock_path);
    rt.spawn(async move {
        if let Err(e) = listener.listen(tx).await {
            eprintln!("Socket error: {}", e);
        }
    });

    // Setup terminal
    enable_raw_mode()?;
    std::io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let renderer = TreeViewRenderer;
    let mut tree = AgentTree::new();
    let mut scroll_offset: usize = 0;

    loop {
        // Drain events from socket
        while let Ok(ev) = rx.try_recv() {
            apply_event(&mut tree, ev);
        }

        // Render
        terminal.draw(|frame| {
            renderer.render(&tree, frame, scroll_offset);
        })?;

        // Handle input (100ms timeout = ~10fps)
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Up | KeyCode::Char('k') => {
                        scroll_offset = scroll_offset.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        scroll_offset += 1;
                    }
                    KeyCode::Char('c') => {
                        // TODO: collapse/expand selected agent
                    }
                    _ => {}
                }
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    std::io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
