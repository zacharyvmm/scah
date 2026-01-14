mod fake;
mod header;
mod rust;

pub use header::{QueryError, ROOT, Store};
pub use rust::Element;
pub(crate) use rust::RustStore;
pub use rust::{Child, ChildIndex};

pub(crate) use fake::FakeStore;
