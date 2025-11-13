mod parser;
mod query;

pub(crate) use query::tree::MatchTree;
pub(crate) use query::manager::DocumentPosition;
pub use query::tree::Node;
pub use query::manager::FsmManager;
pub use parser::tree::{SelectionPart, SelectionTree, SelectionKind, Save};