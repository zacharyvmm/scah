//! Runs the Rust code blocks in the project README and the guides in `docs/`
//! as doctests, so the examples in the documentation keep compiling and
//! running: `cargo test -p scah-doctests`.
//!
//! This lives in its own unpublished crate because the published `scah`
//! package can't include files from outside its own directory.

#[cfg(doctest)]
#[doc = include_str!("../../../README.md")]
pub struct Readme;

#[cfg(doctest)]
#[doc = include_str!("../../../docs/selectors.md")]
pub struct Selectors;

#[cfg(doctest)]
#[doc = include_str!("../../../docs/text-extraction.md")]
pub struct TextExtraction;

#[cfg(doctest)]
#[doc = include_str!("../../../docs/architecture.md")]
pub struct Architecture;
