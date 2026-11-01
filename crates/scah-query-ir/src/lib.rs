//! The query layer of [scah](https://docs.rs/scah): CSS selector parsing,
//! the query builders, and the compiled program the matcher runs.
//!
//! ```text
//! query::selector   CSS selector lexer and parser, into element predicates
//! query::compiler   Query, QueryBuilder, Save, and the query! IR
//! program           every query of a parse compiled into one bitset automaton
//! ```
//!
//! The types users need, such as [`Query`], [`Save`], and
//! [`SelectorParseError`], are re-exported by scah.
//!
//! This crate is an implementation detail of scah and has no stability
//! guarantees of its own; use scah instead.

pub mod program;
mod query;

pub use program::{
    LaneSource, Program, ProgramFeatures, SectionIndex, SectionMask, StepMask, StepPlan,
};
pub use query::compiler::lazy;
pub use query::compiler::{
    AttributeNames, PredicateMetadata, Query, QueryBuilder, QueryFactory, QuerySection,
    QuerySectionId, QuerySpec, Save, SelectionKind, SelectorParseError, StaticQuery,
    TextRequirements, Transition, TransitionId, ascii_case_insensitive_hash,
};
pub use query::selector::{
    AnPlusB, AttributeCaseSensitivity, AttributeSelection, AttributeSelectionKind,
    AttributeSelections, ClassSelections, Combinator, ElementPredicate, IElement,
    LocalLogicalPredicate, LocalSelectorList, LogicalPredicates, MAX_SELECTOR_NESTING_DEPTH,
    StructuralMatchContext, StructuralPredicate, StructuralPredicates,
};
