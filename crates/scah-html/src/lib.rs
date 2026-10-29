//! The HTML layer of [scah](https://docs.rs/scah): finding tags in a
//! document, reading their attributes, and tracking which elements are open.
//!
//! ```text
//! reader      a byte cursor over the document
//! indexer     the next open or close tag, comment or raw-text end (SIMD)
//! element     the attributes of an open tag, only those asked for
//! tag_id      tag names resolved to a one-byte TagId
//! tag         what HTML says about a tag: void, raw text, implied closes
//! tree        the open elements, and the rules that close them
//! ```
//!
//! This crate is an implementation detail of scah and has no stability
//! guarantees of its own; use scah instead.

mod attribute_interest;
mod attributes;
mod element;
mod indexer;
mod reader;
mod simd_classifier;
mod tag;
mod tag_id;
mod tree;

pub use attribute_interest::{AttributeInterest, AttributeMask};
pub use element::{Attribute, XHtmlElement};
pub use indexer::{AutoTagIndexer, IndexingMode, OpenTagStart, TagEvent, TagIndexer, TagKind};
pub use reader::Reader;
#[doc(hidden)]
pub use simd_classifier::BlockClassifier;
pub use tag::{ClassifiedTag, ScopeKind, TagFlags, TextTagFlags};
pub use tag_id::TagId;
pub use tree::{DepthExceeded, DepthSize, MAX_ELEMENT_DEPTH, OpenElement, OpenElementStack};
