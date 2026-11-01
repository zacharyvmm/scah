#[inline]
pub(crate) fn is_css_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0C)
}

#[inline]
pub(crate) fn is_css_whitespace_char(character: char) -> bool {
    character.is_ascii() && is_css_whitespace(character as u8)
}

/// Trims CSS whitespace from both ends of `source`, returning the byte offset
/// of the trimmed text in `source` along with it.
pub(crate) fn trim_css_whitespace(source: &str) -> (usize, &str) {
    let start = source.len() - source.trim_start_matches(is_css_whitespace_char).len();
    (
        start,
        source[start..].trim_end_matches(is_css_whitespace_char),
    )
}

mod builder;
mod eq;
mod lexer;
mod scan;
mod string_search;

pub use builder::{
    AnPlusB, AttributeSelection, AttributeSelections, ClassSelections, ElementPredicate, IElement,
    LocalLogicalPredicate, LocalSelectorList, LogicalPredicates, MAX_SELECTOR_NESTING_DEPTH,
    StructuralMatchContext, StructuralPredicate, StructuralPredicates,
};
pub use lexer::Combinator;
pub(super) use lexer::Lexer;
pub(crate) use scan::{require_modeled_syntax, split_selector_list};
pub use string_search::{AttributeCaseSensitivity, AttributeSelectionKind};
