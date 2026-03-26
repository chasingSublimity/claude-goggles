pub mod tree_view;
pub mod bloom;
pub mod footer;

use ratatui::Frame;
use crate::model::AgentTree;

pub trait Renderer {
    fn render(&mut self, tree: &AgentTree, frame: &mut Frame, scroll_offset: usize, selected: usize);
}
