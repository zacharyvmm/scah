pub(crate) mod attribute_interest;
pub mod cursor;
pub mod executor;
pub(crate) mod frontier;
pub mod multiplexer;
//pub mod tree;

pub(crate) type DepthSize = u16;

/// Maximum real element depth; [`DepthSize::MAX`] is reserved for cursor sentinels.
pub(crate) const MAX_ELEMENT_DEPTH: DepthSize = DepthSize::MAX - 1;
