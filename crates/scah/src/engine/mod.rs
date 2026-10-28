pub(crate) mod attribute_interest;
pub(crate) mod matcher;
mod scopes;
pub(crate) mod structural;

pub(crate) type DepthSize = u16;

/// Maximum real element depth.
pub(crate) const MAX_ELEMENT_DEPTH: DepthSize = DepthSize::MAX - 1;
