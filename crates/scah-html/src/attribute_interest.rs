/// Attributes to tokenize, as bits over a list of attribute names.
///
/// `id` and `class` have dedicated flags because the parser stores them in
/// dedicated element fields.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AttributeMask {
    pub flags: u8,
    /// Bit `i` stands for the `i`th name of the [`AttributeInterest`].
    pub keys: u64,
}

impl AttributeMask {
    pub const ID: u8 = 1;
    pub const CLASS: u8 = 1 << 1;
    /// Every attribute: the step saves them, or names overflowed `keys`.
    pub const ALL: u8 = 1 << 2;

    #[inline(always)]
    pub fn union(self, other: Self) -> Self {
        Self {
            flags: self.flags | other.flags,
            keys: self.keys | other.keys,
        }
    }

    #[inline(always)]
    pub fn is_empty(self) -> bool {
        self.flags == 0 && self.keys == 0
    }
}

/// Normalized text checks `hidden` on every tag, independently of selectors.
const HIDDEN: u8 = 1 << 7;

/// Attributes the parser must tokenize for the current opening tag.
///
/// Callers contribute precomputed [`AttributeMask`]s, so building the
/// interest for a tag is a few `OR`s.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AttributeInterest<'query> {
    mask: AttributeMask,
    /// The attribute names, indexed by the bits of `mask.keys`.
    names: Box<[&'query str]>,
}

impl<'query> AttributeInterest<'query> {
    pub fn new(names: &[&'query str]) -> Self {
        Self {
            mask: AttributeMask::default(),
            names: names.into(),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.mask = AttributeMask::default();
    }

    #[inline]
    pub fn require_hidden(&mut self) {
        self.mask.flags |= HIDDEN;
    }

    #[inline(always)]
    pub fn add(&mut self, mask: AttributeMask) {
        self.mask = self.mask.union(mask);
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.mask.is_empty()
    }

    #[inline]
    fn all(&self) -> bool {
        self.mask.flags & AttributeMask::ALL != 0
    }

    #[inline]
    pub fn includes_id(&self) -> bool {
        self.mask.flags & (AttributeMask::ALL | AttributeMask::ID) != 0
    }

    #[inline]
    pub fn includes_class(&self) -> bool {
        self.mask.flags & (AttributeMask::ALL | AttributeMask::CLASS) != 0
    }

    #[inline]
    pub fn includes_attribute(&self, key: &str) -> bool {
        if self.all() || (self.mask.flags & HIDDEN != 0 && key.eq_ignore_ascii_case("hidden")) {
            return true;
        }
        let mut keys = self.mask.keys;
        while keys != 0 {
            let index = keys.trailing_zeros() as usize;
            keys &= keys - 1;
            if self.names[index].eq_ignore_ascii_case(key) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_is_tracked_without_other_attributes() {
        let mut interest = AttributeInterest::new(&[]);
        interest.require_hidden();
        interest.require_hidden();

        assert!(interest.includes_attribute("HIDDEN"));
        assert!(interest.includes_attribute("hidden"));
        assert!(!interest.includes_attribute("data-unused"));
        assert!(!interest.includes_id());
        assert!(!interest.is_empty());
    }
}
