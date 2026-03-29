mod query;

pub use query::compiler::lazy;
pub use query::compiler::{
    Position, Query, QueryBuilder, QueryFactory, QuerySection, QuerySpec, Save, SelectionKind,
    SelectorParseError, StaticQuery, Transition,
};
pub use query::selector::{
    Attribute, AttributeSelection, AttributeSelectionKind, AttributeSelections, Combinator,
    ElementPredicate, IElement,
};
pub use scah_reader::Reader;
