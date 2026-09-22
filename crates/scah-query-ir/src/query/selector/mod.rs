#[inline]
pub(crate) fn is_css_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0C)
}

#[inline]
pub(crate) fn is_css_whitespace_char(character: char) -> bool {
    character.is_ascii() && is_css_whitespace(character as u8)
}

mod builder;
mod eq;
mod lexer;
mod string_search;

pub use builder::{
    AnPlusB, Attribute, AttributeSelection, AttributeSelections, ClassSelections, ElementPredicate,
    IElement, LocalLogicalPredicate, LocalSelectorList, LogicalPredicates,
    MAX_SELECTOR_NESTING_DEPTH, StructuralMatchContext, StructuralPredicate, StructuralPredicates,
};
pub use lexer::Combinator;
pub(super) use lexer::Lexer;
pub use string_search::{AttributeCaseSensitivity, AttributeSelectionKind};
