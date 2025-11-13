mod selection;
mod tree;

pub use selection::{Save, SelectionKind, SelectionPart};
pub use tree::SelectionTree;
pub(crate) use tree::{NextPosition, Position};