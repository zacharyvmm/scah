//! Finding tags in the source: tag boundaries (`indexer`, backed by a SIMD
//! byte classifier) and the attributes of open tags (`element`).

mod attributes;
pub mod element;
pub(crate) mod indexer;
mod simd_classifier;

#[cfg(feature = "simd-bench-internals")]
pub(crate) use simd_classifier::BlockClassifier;
