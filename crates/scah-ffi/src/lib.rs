//! Foreign-language support for [`scah`].
//!
//! [`scah::Store`] borrows both the HTML it was parsed from and the selector
//! strings of its queries. That is ideal in Rust, but a Python, Node, or C
//! caller cannot express those borrows. The [`owned`] module packages each
//! borrowed value together with the allocation it borrows from, so foreign
//! bindings can hold queries and stores for as long as they like.
//!
//! The [`arrow`] module exports query results as Apache Arrow columns through
//! the Arrow C Data Interface, borrowing strings from the store instead of
//! copying them.

pub mod arrow;
pub mod owned;

pub use owned::{OwnedQuery, OwnedStore};
