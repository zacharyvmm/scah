mod fake;
mod header;
mod rust;
pub use header::QueryError;
pub use header::Store;
pub use rust::{Element, SelectionValue};
pub(crate) use rust::{RustStore, ValueKind};

pub(crate) use fake::FakeStore;
