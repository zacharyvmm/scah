use super::builder::{AttributeSelection, ElementPredicate, IElement};
use super::string_search::{AttributeSelectionKind, css_words};
use scah_html::Attribute;
use scah_html::entities::{decode_attribute_value, value_needs_decoding};

impl<'a> AttributeSelection<'a> {
    /// Whether `other`, as written in the HTML, satisfies this selector.
    /// Its value is compared decoded, as
    /// [`decode_attribute_value`] decodes it.
    pub fn matches_attribute(&self, other: &Attribute<'_>) -> bool {
        // Attribute names are case-insensitive in HTML.
        if !self.name.eq_ignore_ascii_case(other.key) {
            return false;
        }

        if self.value.is_none() || self.kind == AttributeSelectionKind::Presence {
            return true;
        }

        let Some(value) = other.value else {
            return false;
        };
        self.matches_value(value)
    }

    /// Match this selector against a value routed through one of the
    /// dedicated element fields (`id` / `class`), which are stored separately
    /// from the generic attribute list. A missing field never matches.
    fn matches_field(&self, field: Option<&str>) -> bool {
        let Some(value) = field else {
            return false;
        };

        if self.value.is_none() || self.kind == AttributeSelectionKind::Presence {
            return true;
        }

        self.matches_value(value)
    }

    /// Compare the selector's value, by its operator, with `value` as
    /// written in the HTML, once decoded.
    #[inline]
    fn matches_value(&self, value: &str) -> bool {
        if value_needs_decoding(value) {
            return self.matches_decoded_value(value);
        }
        self.compare_value(value)
    }

    /// [`Self::matches_value`] for a value with `&`, CR, or NUL, which may
    /// need decoding.
    #[cold]
    #[inline(never)]
    fn matches_decoded_value(&self, value: &str) -> bool {
        self.compare_value(&decode_attribute_value(value))
    }

    /// Compare the selector's value, by its operator, with `value`.
    #[inline]
    fn compare_value(&self, value: &str) -> bool {
        let query = self.value.unwrap_or_default();
        match self.case_sensitivity {
            super::string_search::AttributeCaseSensitivity::AsciiInsensitive => {
                self.kind.find_ascii_insensitive(query, value)
            }
            _ => self.kind.find(query, value),
        }
    }
}

impl<'a> ElementPredicate<'a> {
    /// Whether the tag name alone can rule this predicate out.
    #[inline]
    pub fn matches_name(&self, name: &str) -> bool {
        self.name
            .is_none_or(|expected| expected.eq_ignore_ascii_case(name))
    }

    /// Whether evaluating this predicate needs `id`, `class`, or generic
    /// attributes in addition to the element name.
    #[inline]
    pub fn requires_attributes(&self) -> bool {
        self.id.is_some()
            || !self.classes.as_slice().is_empty()
            || !self.attributes.as_slice().is_empty()
            || self.logical.as_slice().iter().any(|logical| match logical {
                super::builder::LocalLogicalPredicate::Not(list)
                | super::builder::LocalLogicalPredicate::Any(list) => list
                    .as_slice()
                    .iter()
                    .any(ElementPredicate::requires_attributes),
            })
            || self
                .structural
                .as_slice()
                .iter()
                .any(|structural| match structural {
                    super::builder::StructuralPredicate::NthChildOf(_, filter) => filter
                        .as_slice()
                        .iter()
                        .any(ElementPredicate::requires_attributes),
                    _ => false,
                })
    }

    pub fn requires_structural(&self) -> bool {
        !self.structural.as_slice().is_empty()
            || self.logical.as_slice().iter().any(|logical| {
                let list = match logical {
                    super::builder::LocalLogicalPredicate::Not(list)
                    | super::builder::LocalLogicalPredicate::Any(list) => list.as_slice(),
                };
                list.iter().any(ElementPredicate::requires_structural)
            })
    }

    fn matches_classes(&self, element_classes: &str) -> bool {
        let selector_classes = self.classes.as_slice();
        match selector_classes.len() {
            0 => true,
            1 => css_words(element_classes).any(|word| word == selector_classes[0]),
            len if len <= u64::BITS as usize => {
                let mut matched = 0_u64;

                for word in css_words(element_classes) {
                    for (index, selector_class) in selector_classes.iter().enumerate() {
                        if word == *selector_class {
                            matched |= 1 << index;
                        }
                    }
                }

                matched.count_ones() as usize == len
            }
            _ => {
                let mut matched = vec![false; selector_classes.len()];

                for word in css_words(element_classes) {
                    for (index, selector_class) in selector_classes.iter().enumerate() {
                        if !matched[index] && word == *selector_class {
                            matched[index] = true;
                        }
                    }
                }

                matched.into_iter().all(std::convert::identity)
            }
        }
    }

    /// Match against an element without structural context.
    ///
    /// Predicates that require structural context, directly or nested in
    /// `:is()`/`:not()`, never match here.
    #[inline]
    pub fn matches_element<'b, E: IElement<'b>>(&self, other: &E) -> bool {
        self.matches_element_with_context(other, None)
    }

    /// Ordinary compounds (no logical or structural predicates) are decided
    /// entirely by the inline, non-recursive local checks; only compounds that
    /// carry `:is()`/`:not()`/structural predicates fall through to the
    /// out-of-line recursive evaluator.
    #[inline(always)]
    pub fn matches_element_with_context<'b, E: IElement<'b>>(
        &self,
        other: &E,
        structural: Option<&super::builder::StructuralMatchContext<'_>>,
    ) -> bool {
        self.matches_local(other)
            && (self.is_ordinary() || self.matches_extensions(other, structural))
    }

    /// [`Self::matches_element_with_context`] for an element whose name is
    /// already known to match, as after a matcher's name lookup.
    #[doc(hidden)]
    #[inline(always)]
    pub fn matches_named_element_with_context<'b, E: IElement<'b>>(
        &self,
        other: &E,
        structural: Option<&super::builder::StructuralMatchContext<'_>>,
    ) -> bool {
        debug_assert!(self.matches_name(other.name()));
        self.matches_local_after_name(other)
            && (self.is_ordinary() || self.matches_extensions(other, structural))
    }

    /// Match the element-local portion of this predicate without structural
    /// context. Streaming engines may use this for prevalidated local lists.
    #[doc(hidden)]
    #[inline(always)]
    pub fn matches_local_element_unchecked<'b, E: IElement<'b>>(&self, other: &E) -> bool {
        self.matches_local(other)
            && (self.logical.as_slice().is_empty() || self.matches_local_logical_unchecked(other))
    }

    /// Whether this compound has no logical or structural predicates.
    #[inline(always)]
    fn is_ordinary(&self) -> bool {
        self.logical.as_slice().is_empty() && self.structural.as_slice().is_empty()
    }

    /// Name, id, class, and attribute checks. Never recurses.
    #[inline(always)]
    fn matches_local<'b, E: IElement<'b>>(&self, other: &E) -> bool {
        self.matches_name(other.name()) && self.matches_local_after_name(other)
    }

    /// [`Self::matches_classes`] for a class value as written, once
    /// decoded. Selector classes have no `&`, CR, or NUL, and a word without
    /// them decodes to itself, so this only finds more when the value has
    /// one.
    #[cold]
    #[inline(never)]
    fn matches_decoded_classes(&self, element_classes: &str) -> bool {
        value_needs_decoding(element_classes)
            && self.matches_classes(&decode_attribute_value(element_classes))
    }

    /// Id, class, and attribute checks. Never recurses.
    ///
    /// Values are compared decoded, as [`decode_attribute_value`] decodes
    /// them. Ids and classes are compared as written first, which is enough
    /// unless they have `&`, CR, or NUL, since selector ids and classes
    /// cannot.
    #[inline(always)]
    fn matches_local_after_name<'b, E: IElement<'b>>(&self, other: &E) -> bool {
        if let Some(id) = self.id {
            match other.id() {
                Some(element_id) if element_id == id || decoded_id_matches(element_id, id) => {}
                _ => return false,
            }
        }

        if !self.classes.as_slice().is_empty() {
            let Some(element_classes) = other.class() else {
                return false;
            };

            if !self.matches_classes(element_classes)
                && !self.matches_decoded_classes(element_classes)
            {
                return false;
            }
        }

        let attributes = self.attributes.as_slice();
        attributes.is_empty()
            || attributes.iter().all(|selector_attribute| {
                // `id` and `class` have dedicated element fields (the first
                // of each with a value), so `[id]`/`[class]` selectors check
                // those first. Attribute names are case-insensitive in HTML.
                // Valueless or duplicate `id`/`class` attributes are matched
                // by the fallback scan of the attribute list.
                if selector_attribute.name.eq_ignore_ascii_case("id") {
                    selector_attribute.matches_field(other.id())
                        || other
                            .attributes()
                            .iter()
                            .any(|attribute| selector_attribute.matches_attribute(attribute))
                } else if selector_attribute.name.eq_ignore_ascii_case("class") {
                    selector_attribute.matches_field(other.class())
                        || other
                            .attributes()
                            .iter()
                            .any(|attribute| selector_attribute.matches_attribute(attribute))
                } else {
                    other.attributes().iter().any(|xhtml_attribute| {
                        selector_attribute.matches_attribute(xhtml_attribute)
                    })
                }
            })
    }

    /// Recursive evaluation of logical predicates for prevalidated local
    /// lists, kept out of line so it does not bloat the local hot path.
    #[inline(never)]
    fn matches_local_logical_unchecked<'b, E: IElement<'b>>(&self, other: &E) -> bool {
        self.logical.as_slice().iter().all(|logical| match logical {
            super::builder::LocalLogicalPredicate::Not(list) => !list
                .as_slice()
                .iter()
                .any(|predicate| predicate.matches_local_element_unchecked(other)),
            super::builder::LocalLogicalPredicate::Any(list) => list
                .as_slice()
                .iter()
                .any(|predicate| predicate.matches_local_element_unchecked(other)),
        })
    }

    /// Recursive evaluation of logical and structural predicates, kept out
    /// of line so it does not bloat the ordinary matching hot path.
    #[inline(never)]
    fn matches_extensions<'b, E: IElement<'b>>(
        &self,
        other: &E,
        structural: Option<&super::builder::StructuralMatchContext<'_>>,
    ) -> bool {
        // Without context, a structural predicate nested under `:not()` must
        // not turn into a match, so reject the whole compound up front.
        if structural.is_none() && self.requires_structural() {
            return false;
        }

        self.logical.as_slice().iter().all(|logical| match logical {
            super::builder::LocalLogicalPredicate::Not(list) => !list
                .as_slice()
                .iter()
                .any(|predicate| predicate.matches_element_with_context(other, structural)),
            super::builder::LocalLogicalPredicate::Any(list) => list
                .as_slice()
                .iter()
                .any(|predicate| predicate.matches_element_with_context(other, structural)),
        }) && self.structural.as_slice().iter().all(|predicate| {
            let Some(context) = structural else {
                return false;
            };
            match predicate {
                super::builder::StructuralPredicate::Root => context.is_document_root,
                super::builder::StructuralPredicate::Scope => context.is_scope_root,
                super::builder::StructuralPredicate::FirstChild => context.child_index == 1,
                super::builder::StructuralPredicate::NthChild(formula) => {
                    formula.matches(context.child_index)
                }
                super::builder::StructuralPredicate::FirstOfType => context.type_index == 1,
                super::builder::StructuralPredicate::NthOfType(formula) => {
                    formula.matches(context.type_index)
                }
                super::builder::StructuralPredicate::NthChildOf(formula, filter) => context
                    .filtered_child_indices
                    .iter()
                    .any(|&(context_filter, index)| {
                        std::ptr::eq(context_filter, filter) && formula.matches(index)
                    }),
            }
        })
    }
}

/// Whether `element_id`, as written, decodes to `id`.
#[cold]
#[inline(never)]
fn decoded_id_matches(element_id: &str, id: &str) -> bool {
    value_needs_decoding(element_id) && *decode_attribute_value(element_id) == *id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AttributeSelections, ClassSelections};

    #[derive(Debug)]
    struct FakeElement<'a> {
        name: &'a str,
        id: Option<&'a str>,
        class: Option<&'a str>,
        attributes: &'a [Attribute<'a>],
    }

    impl<'a> IElement<'a> for FakeElement<'a> {
        fn name(&self) -> &'a str {
            self.name
        }

        fn id(&self) -> Option<&'a str> {
            self.id
        }

        fn class(&self) -> Option<&'a str> {
            self.class
        }

        fn attributes(&self) -> &[Attribute<'a>] {
            self.attributes
        }
    }

    #[test]
    fn test_attribute_selection_comparison() {
        assert!(
            AttributeSelection {
                name: "hello",
                value: Some("World"),
                kind: AttributeSelectionKind::Exact,
                case_sensitivity: crate::AttributeCaseSensitivity::Default,
            }
            .matches_attribute(&Attribute {
                key: "hello",
                value: Some("World")
            })
        );
    }

    #[test]
    fn attribute_selections_compare_decoded_values() {
        let selection = |value| AttributeSelection {
            name: "title",
            value: Some(value),
            kind: AttributeSelectionKind::Exact,
            case_sensitivity: crate::AttributeCaseSensitivity::Default,
        };
        let attribute = Attribute {
            key: "title",
            value: Some("a&amp;b"),
        };
        assert!(selection("a&b").matches_attribute(&attribute));
        assert!(!selection("a&amp;b").matches_attribute(&attribute));
    }

    #[test]
    fn attribute_selections_compare_values_with_crs_and_nuls_normalized() {
        let selection = |value| AttributeSelection {
            name: "title",
            value: Some(value),
            kind: AttributeSelectionKind::Exact,
            case_sensitivity: crate::AttributeCaseSensitivity::Default,
        };
        let attribute = |value| Attribute {
            key: "title",
            value: Some(value),
        };
        assert!(selection("a\nb\n\u{FFFD}").matches_attribute(&attribute("a\r\nb\r\0")));
        assert!(!selection("a\r\nb\r\0").matches_attribute(&attribute("a\r\nb\r\0")));
        assert!(selection("a\rb").matches_attribute(&attribute("a&#13;b")));
        assert!(!selection("a\rb").matches_attribute(&attribute("a\rb")));
    }

    #[test]
    fn element_values_are_decoded() {
        let predicate = ElementPredicate {
            name: Some("p"),
            id: Some("a"),
            classes: ClassSelections::from_static(&["x", "y", "z\u{FFFD}"]),
            attributes: AttributeSelections::from(vec![AttributeSelection {
                name: "lang",
                value: Some("ab"),
                kind: AttributeSelectionKind::Exact,
                case_sensitivity: crate::AttributeCaseSensitivity::Default,
            }]),
            logical: crate::LogicalPredicates::from_static(&[]),
            structural: crate::StructuralPredicates::from_static(&[]),
        };
        let element = FakeElement {
            name: "p",
            id: Some("&#x61;"),
            class: Some("x&#32;y\r\nz\0"),
            attributes: &[Attribute {
                key: "lang",
                value: Some("a&#x62;"),
            }],
        };
        assert!(predicate.matches_element(&element));
    }

    #[test]
    fn test_element_selection_comparison() {
        assert!(
            ElementPredicate {
                name: Some("hello"),
                id: Some("id"),
                classes: ClassSelections::from_static(&["world"]),
                attributes: AttributeSelections::from(vec![AttributeSelection {
                    name: "selected",
                    value: Some("true"),
                    kind: AttributeSelectionKind::Exact,
                    case_sensitivity: crate::AttributeCaseSensitivity::Default
                }]),
                logical: crate::LogicalPredicates::from_static(&[]),
                structural: crate::StructuralPredicates::from_static(&[]),
            }
            .matches_element(&FakeElement {
                name: "hello",
                id: Some("id"),
                class: Some("hello world"),
                attributes: &[
                    Attribute {
                        key: "key1",
                        value: Some("value1")
                    },
                    Attribute {
                        key: "key2",
                        value: Some("value2")
                    },
                    Attribute {
                        key: "selected",
                        value: Some("true")
                    },
                ]
            })
        );
    }

    #[test]
    fn test_realistic_search() {
        assert!(
            ElementPredicate {
                name: Some("a"),
                id: None,
                classes: ClassSelections::from_static(&["underline-green"]),
                attributes: AttributeSelections::from(vec![AttributeSelection {
                    name: "href",
                    value: None,
                    kind: AttributeSelectionKind::Presence,
                    case_sensitivity: crate::AttributeCaseSensitivity::Default
                }]),
                logical: crate::LogicalPredicates::from_static(&[]),
                structural: crate::StructuralPredicates::from_static(&[]),
            }
            .matches_element(&FakeElement {
                name: "a",
                id: Some("search-link"),
                class: Some("text-white underline-green p-4"),
                attributes: &[
                    Attribute {
                        key: "key1",
                        value: Some("value1")
                    },
                    Attribute {
                        key: "href",
                        value: Some("/search")
                    },
                    Attribute {
                        key: "selected",
                        value: Some("true")
                    },
                ]
            })
        );
    }

    #[test]
    fn test_multiple_class_selection_comparison() {
        assert!(
            ElementPredicate {
                name: Some("a"),
                id: None,
                classes: ClassSelections::from_static(&["blue", "exit"]),
                attributes: AttributeSelections::from_static(&[]),
                logical: crate::LogicalPredicates::from_static(&[]),
                structural: crate::StructuralPredicates::from_static(&[])
            }
            .matches_element(&FakeElement {
                name: "a",
                id: None,
                class: Some("blue large exit"),
                attributes: &[],
            })
        );
    }

    #[test]
    fn test_multiple_class_selection_comparison_is_order_independent() {
        assert!(
            ElementPredicate {
                name: Some("a"),
                id: None,
                classes: ClassSelections::from_static(&["exit", "blue"]),
                attributes: AttributeSelections::from_static(&[]),
                logical: crate::LogicalPredicates::from_static(&[]),
                structural: crate::StructuralPredicates::from_static(&[])
            }
            .matches_element(&FakeElement {
                name: "a",
                id: None,
                class: Some("blue large exit"),
                attributes: &[],
            })
        );
    }

    #[test]
    fn test_multiple_class_selection_comparison_requires_all_classes() {
        assert!(
            !ElementPredicate {
                name: Some("a"),
                id: None,
                classes: ClassSelections::from_static(&["blue", "exit", "missing"]),
                attributes: AttributeSelections::from_static(&[]),
                logical: crate::LogicalPredicates::from_static(&[]),
                structural: crate::StructuralPredicates::from_static(&[])
            }
            .matches_element(&FakeElement {
                name: "a",
                id: None,
                class: Some("blue large exit"),
                attributes: &[],
            })
        );
    }

    #[test]
    fn test_class_matching_is_order_independent_for_selector_and_element() {
        let selector_one = ElementPredicate {
            name: Some("a"),
            id: None,
            classes: ClassSelections::from_static(&["blue", "exit"]),
            attributes: AttributeSelections::from_static(&[]),
            logical: crate::LogicalPredicates::from_static(&[]),
            structural: crate::StructuralPredicates::from_static(&[]),
        };
        let selector_two = ElementPredicate {
            name: Some("a"),
            id: None,
            classes: ClassSelections::from_static(&["exit", "blue"]),
            attributes: AttributeSelections::from_static(&[]),
            logical: crate::LogicalPredicates::from_static(&[]),
            structural: crate::StructuralPredicates::from_static(&[]),
        };

        let element_one = FakeElement {
            name: "a",
            id: None,
            class: Some("blue exit"),
            attributes: &[],
        };
        let element_two = FakeElement {
            name: "a",
            id: None,
            class: Some("exit blue"),
            attributes: &[],
        };

        assert!(selector_one.matches_element(&element_one));
        assert!(selector_one.matches_element(&element_two));
        assert!(selector_two.matches_element(&element_one));
        assert!(selector_two.matches_element(&element_two));
    }

    #[test]
    fn class_matching_splits_only_on_css_whitespace() {
        let single = ElementPredicate {
            name: None,
            id: None,
            classes: ClassSelections::from_static(&["foo"]),
            attributes: AttributeSelections::from_static(&[]),
            logical: crate::LogicalPredicates::from_static(&[]),
            structural: crate::StructuralPredicates::from_static(&[]),
        };
        let multiple = ElementPredicate {
            classes: ClassSelections::from_static(&["bar", "foo"]),
            ..single.clone()
        };
        let element = |class| FakeElement {
            name: "div",
            id: None,
            class: Some(class),
            attributes: &[],
        };

        for class in ["bar foo", "bar\tfoo", "bar\u{000C}foo", "\r\nbar\n\rfoo\t"] {
            assert!(single.matches_element(&element(class)), "{class:?}");
            assert!(multiple.matches_element(&element(class)), "{class:?}");
        }
        for class in ["bar\u{00A0}foo", "bar\u{000B}foo", "bar\u{2003}foo"] {
            assert!(!single.matches_element(&element(class)), "{class:?}");
            assert!(!multiple.matches_element(&element(class)), "{class:?}");
        }
    }

    #[test]
    fn logical_structural_predicates_require_and_use_context() {
        let structural = ElementPredicate {
            name: Some("div"),
            id: None,
            classes: ClassSelections::from_static(&[]),
            attributes: AttributeSelections::from_static(&[]),
            logical: crate::LogicalPredicates::from_static(&[]),
            structural: crate::StructuralPredicates::from(vec![
                crate::StructuralPredicate::FirstChild,
            ]),
        };
        let predicate = ElementPredicate {
            name: None,
            id: None,
            classes: ClassSelections::from_static(&[]),
            attributes: AttributeSelections::from_static(&[]),
            logical: crate::LogicalPredicates::from(vec![crate::LocalLogicalPredicate::Any(
                crate::LocalSelectorList::Owned(vec![structural].into_boxed_slice()),
            )]),
            structural: crate::StructuralPredicates::from_static(&[]),
        };
        let element = FakeElement {
            name: "div",
            id: None,
            class: None,
            attributes: &[],
        };
        assert!(!predicate.matches_element(&element));
        assert!(predicate.matches_element_with_context(
            &element,
            Some(&crate::StructuralMatchContext {
                child_index: 1,
                type_index: 1,
                filtered_child_indices: smallvec::SmallVec::new(),
                is_document_root: false,
                is_scope_root: false,
            })
        ));
        assert!(!predicate.matches_element_with_context(
            &element,
            Some(&crate::StructuralMatchContext {
                child_index: 2,
                type_index: 1,
                filtered_child_indices: smallvec::SmallVec::new(),
                is_document_root: false,
                is_scope_root: false,
            })
        ));
    }

    #[test]
    fn root_and_scope_use_independent_context_flags() {
        let element = FakeElement {
            name: "div",
            id: None,
            class: None,
            attributes: &[],
        };
        let make_predicate = |structural| ElementPredicate {
            name: Some("div"),
            id: None,
            classes: ClassSelections::from_static(&[]),
            attributes: AttributeSelections::from_static(&[]),
            logical: crate::LogicalPredicates::from_static(&[]),
            structural: crate::StructuralPredicates::from(vec![structural]),
        };
        let document_root = crate::StructuralMatchContext {
            child_index: 1,
            type_index: 1,
            filtered_child_indices: smallvec::SmallVec::new(),
            is_document_root: true,
            is_scope_root: false,
        };
        let scope_root = crate::StructuralMatchContext {
            child_index: 2,
            type_index: 1,
            filtered_child_indices: smallvec::SmallVec::new(),
            is_document_root: false,
            is_scope_root: true,
        };

        assert!(
            make_predicate(crate::StructuralPredicate::Root)
                .matches_element_with_context(&element, Some(&document_root))
        );
        assert!(
            !make_predicate(crate::StructuralPredicate::Scope)
                .matches_element_with_context(&element, Some(&document_root))
        );
        assert!(
            make_predicate(crate::StructuralPredicate::Scope)
                .matches_element_with_context(&element, Some(&scope_root))
        );
        assert!(
            !make_predicate(crate::StructuralPredicate::Root)
                .matches_element_with_context(&element, Some(&scope_root))
        );
    }
}
