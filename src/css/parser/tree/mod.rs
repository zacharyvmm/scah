mod selection;
mod tree;

pub use selection::{Save, SelectionKind, SelectionPart};
pub use tree::Selection;
pub(crate) use tree::{NextPosition, Position};
