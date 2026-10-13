use scah_query_ir::AttributeMask;

/// Normalized text checks `hidden` on every tag, independently of selectors.
const HIDDEN: u8 = 1 << 7;

/// Attributes the parser must tokenize for the current opening tag.
///
/// Matching steps contribute precomputed [`AttributeMask`]s, so building the
/// interest for a tag is a few `OR`s. Viable save points require every
/// attribute, because the result contract preserves them all.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct AttributeInterest<'query> {
    mask: AttributeMask,
    /// The program's attribute names, indexed by the bits of `mask.keys`.
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
    use crate::{Program, Query, Save};

    fn interest_for(selector: &'static str, save: Save) -> AttributeInterest<'static> {
        let queries: &'static [Query<'static>] =
            Box::leak(Box::new([Query::all(selector, save).unwrap().build()]));
        let program = Box::leak(Box::new(Program::compile(queries)));
        let mut interest = AttributeInterest::new(program.attribute_names());
        for step in 0..program.step_count() {
            interest.add(program.step_interest(step));
        }
        interest
    }

    #[test]
    fn merges_dedicated_and_generic_attribute_requirements() {
        let interest = interest_for("#hero.promoted[href][HREF]", Save::name_only());

        assert!(interest.includes_id());
        assert!(interest.includes_class());
        assert!(interest.includes_attribute("href"));
        assert!(interest.includes_attribute("Href"));
        assert!(!interest.includes_attribute("rel"));
    }

    #[test]
    fn saving_steps_require_every_attribute() {
        let interest = interest_for("a[href]", Save::none());

        assert!(interest.includes_id());
        assert!(interest.includes_class());
        assert!(interest.includes_attribute("anything"));
    }

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

    #[test]
    fn many_attribute_names_stay_selective() {
        let interest = interest_for("a[href][target][rel][download][data-id]", Save::name_only());

        for name in ["href", "target", "rel", "download", "data-id"] {
            assert!(interest.includes_attribute(name), "{name}");
        }
        assert!(!interest.includes_attribute("title"));
        assert!(!interest.includes_id());
    }

    #[test]
    fn clear_resets_the_mask() {
        let mut interest = interest_for("a[href]", Save::name_only());
        interest.clear();
        assert!(interest.is_empty());
        assert!(!interest.includes_attribute("href"));
    }
}
