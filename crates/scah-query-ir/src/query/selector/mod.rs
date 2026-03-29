mod builder;
mod eq;
mod lexer;
mod string_search;

pub use builder::{
    Attribute, AttributeSelection, AttributeSelections, ClassSelections, ElementPredicate, IElement,
};
pub use lexer::Combinator;
pub(super) use lexer::Lexer;
pub use string_search::AttributeSelectionKind;
