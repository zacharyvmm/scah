pub mod element;
mod entities;
mod indexer;
mod open_elements;
pub mod parser;
mod resolved_indexer;
mod simd_classifier;
pub mod tag;
mod tape_indexer;
mod text_edge;
mod text_state;

pub(crate) use indexer::IndexingMode;
#[cfg(scah_resolved_stats)]
pub use resolved_indexer::stats as resolved_stats;
#[cfg(feature = "simd-bench-internals")]
pub(crate) use simd_classifier::BlockClassifier;
#[cfg(feature = "bench-internals")]
pub use text_state::TextPathStats;
