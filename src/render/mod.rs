pub mod tree_view;
pub mod bloom;
pub mod footer;

use ratatui::Frame;
use crate::model::AgentTree;

/// Trait for visualization backends that render an `AgentTree` to a terminal frame.
pub(crate) trait Renderer {
    fn render(&mut self, tree: &AgentTree, frame: &mut Frame, scroll_offset: usize, selected: usize);
}
