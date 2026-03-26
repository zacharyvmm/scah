mod builder;
mod eq;
mod lexer;
mod string_search;

pub(crate) use builder::ElementPredicate;

pub(crate) use lexer::Combinator;
pub(super) use lexer::Lexer;

#[cfg(test)]
pub(crate) use builder::AttributeSelection;
#[cfg(test)]
pub(crate) use string_search::AttributeSelectionKind;
