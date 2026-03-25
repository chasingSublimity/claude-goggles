pub mod tree_view;

use ratatui::Frame;
use crate::model::AgentTree;

pub trait Renderer {
    fn render(&self, tree: &AgentTree, frame: &mut Frame, scroll_offset: usize);
}
