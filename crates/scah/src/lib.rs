//! # scah - Streaming CSS-selector-driven HTML extraction
//!
//! **scah** (*scan HTML*) is a high-performance parsing library that bridges the gap
//! between SAX/StAX streaming efficiency and DOM convenience. Instead of building a
//! full DOM or manually tracking parser state, you declare what you want with
//! **CSS selectors**; the library handles the streaming complexity and builds a
//! targeted [`Store`] containing only your selections.
//!
//! ## Highlights
//!
//! | Feature | Detail |
//! |---------|--------|
//! | **Single pass, no DOM** | Selectors are evaluated as the document streams through the parser; only matches are stored |
//! | **Familiar API** | CSS selectors including descendant, child, and sibling combinators |
//! | **Composable queries** | Chain selections with [`QueryBuilder::then`] for hierarchical data extraction |
//! | **Zero-copy** | Element names, attributes, and inner HTML are `&str` slices into the source |
//! | **Multi-language** | Rust core with Python and TypeScript/JavaScript bindings |
//!
//! ## Quick Start
//!
//! ```rust
//! use scah::{Query, Save, parse};
//!
//! let html = r#"
//!     <main>
//!         <section>
//!             <a href="link1">Link 1</a>
//!             <a href="link2">Link 2</a>
//!         </section>
//!     </main>
//! "#;
//!
//! // Build a query: find all <a> tags with an href attribute
//! // that are direct children of a <section> inside <main>.
//! let queries = &[
//!     Query::all("main > section > a[href]", Save::all())
//!         .expect("valid selector")
//!         .build()
//! ];
//!
//! let store = parse(html, queries).expect("parse succeeds");
//!
//! // Iterate over matched elements
//! for element in store.get("main > section > a[href]").unwrap() {
//!     println!("{}: {}", element.name(), element.attribute("href").unwrap());
//! }
//! ```
//!
//! ## Structured Querying with `.then()`
//!
//! Instead of flat filtering, you can nest queries using closures.
//! Child queries only run within the context of their parent match,
//! making extraction of hierarchical relationships both efficient and ergonomic:
//!
//! ```rust
//! use scah::{Query, Save, parse};
//!
//! # let html = "<main><section><a href='x'>Link</a></section></main>";
//! let queries = &[Query::all("main > section", Save::all())
//!     .expect("valid selector")
//!     .then(|section| {
//!         Ok([
//!             section.all("> a[href]", Save::all())?,
//!             section.all("div a", Save::all())?,
//!         ])
//!     })
//!     .expect("valid child selectors")
//!     .build()];
//!
//! let store = parse(html, queries).expect("parse succeeds");
//! ```
//!
//! ## Architecture
//!
//! Internally, scah is composed of the following layers:
//!
//! 1. **[`Reader`]**: A zero-copy byte-level cursor over the HTML source.
//! 2. **CSS selector compiler**: Parses selector strings into [`Query`]
//!    transitions, then compiles every query of a parse into one [`Program`]:
//!    flat step and section tables with one bit per compound selector.
//! 3. **[`XHtmlParser`]**: A streaming parser that emits open/close events and
//!    runs the program as a bitset automaton, one frame per open element.
//! 4. **[`Store`]**: A columnar result set: one row per saved element, one
//!    column per saved field (name, attributes, inner HTML, raw or
//!    normalized text), read through [`ElementRef`] handles.
//!
//! ## Supported CSS Selector Syntax
//!
//! | Syntax | Example | Status |
//! |--------|---------|--------|
//! | **Tag name** | `a`, `div` | Working |
//! | **ID** | `#my-id` | Working |
//! | **Class** | `.my-class` | Working |
//! | **Descendant combinator** | `main section a` | Working |
//! | **Child combinator** | `main > section` | Working |
//! | **Attribute presence** | `a[href]` | Working |
//! | **Attribute exact match** | `a[href="url"]` | Working |
//! | **Attribute prefix** | `a[href^="https"]` | Working |
//! | **Attribute suffix** | `a[href$=".com"]` | Working |
//! | **Attribute substring** | `a[href*="example"]` | Working |
//! | **Adjacent sibling** | `h1 + p` | Coming soon |
//! | **General sibling** | `h1 ~ p` | Coming soon |

use std::borrow::Cow;

pub mod debug;
mod engine;
mod html;
mod store;
mod support;

#[cfg(all(any(debug_assertions, test), feature = "otel"))]
mod otel;

pub use html::element::builder::XHtmlElement;
pub use html::parser::XHtmlParser;
pub use scah_macros::query;
pub use scah_query_ir::lazy;
pub use scah_query_ir::{
    AnPlusB, Attribute, AttributeCaseSensitivity, AttributeSelection, AttributeSelectionKind,
    AttributeSelections, ClassSelections, Combinator, ElementPredicate, IElement,
    LocalLogicalPredicate, LocalSelectorList, LogicalPredicates, MAX_SELECTOR_NESTING_DEPTH,
    Program, ProgramFeatures, Query, QueryBuilder, QueryFactory, QuerySection, QuerySectionId,
    QuerySpec, Save, SelectionKind, SelectorParseError, StaticQuery, StructuralMatchContext,
    StructuralPredicate, StructuralPredicates, TextRequirements, Transition, TransitionId,
};
pub use scah_reader::Reader;
pub use store::{CapacityOptions, ElementId, ElementRef, Elements, HeapUsage, Store};

/// Implementation details referenced by `query!` expansions.
#[doc(hidden)]
pub mod __private {
    pub use scah_query_ir::{AttributeNames, PredicateMetadata, ascii_case_insensitive_hash};
}

/// Internal APIs used by benchmarks.
///
/// Cursor instrumentation is available only with `bench-internals`. SIMD
/// scanner access is available only with `simd-bench-internals`.
#[doc(hidden)]
pub mod bench_internals {
    pub use crate::html::tag::{ScopeKind, TagFlags};

    #[cfg(feature = "simd-bench-internals")]
    use crate::html::BlockClassifier;
    #[cfg(feature = "simd-bench-internals")]
    use crate::html::IndexingMode;
    #[cfg(feature = "bench-internals")]
    pub use crate::html::TextPathStats;
    #[cfg(feature = "simd-bench-internals")]
    use crate::store::Store;
    #[cfg(feature = "simd-bench-internals")]
    use crate::{ParseError, Program, QuerySpec, Reader, XHtmlParser};

    /// Reusable production `<` scanner for delimiter-distance benchmarks.
    #[cfg(feature = "simd-bench-internals")]
    #[derive(Debug, Default)]
    pub struct LessThanScanner {
        classifier: BlockClassifier,
    }

    #[cfg(feature = "simd-bench-internals")]
    impl LessThanScanner {
        #[inline]
        pub fn find(&self, source: &[u8], from: usize) -> Option<usize> {
            self.classifier.find_less_than(source, from)
        }
    }

    /// Parse HTML with a fixed indexer strategy for strategy crossover
    /// benchmarks.
    #[cfg(feature = "simd-bench-internals")]
    pub fn parse_with_indexing_mode<'a: 'query, 'html: 'query, 'query: 'html, Q>(
        html: &'html str,
        queries: &'a [Q],
        full_index: bool,
    ) -> Result<Store<'html, 'query>, ParseError>
    where
        Q: QuerySpec<'query>,
    {
        if queries.is_empty() {
            return Err(ParseError::EmptyQueries);
        }

        let indexing_mode = if full_index {
            IndexingMode::ForcedFullDocument
        } else {
            IndexingMode::Rolling
        };
        let mut parser = XHtmlParser::from_program(
            std::borrow::Cow::Owned(Program::compile(queries)),
            Some(html.len()),
            Some(indexing_mode),
        );
        let mut reader = Reader::new(html);
        while parser.next(&mut reader) {}

        if let Some(err) = parser.take_parse_error() {
            return Err(err);
        }

        Ok(parser.finish())
    }
}

/// Errors that can occur during parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The query slice passed to [`parse`] is empty.
    EmptyQueries,
    /// The open-element stack exceeded the maximum supported nesting depth
    /// (`u16::MAX - 1` elements).
    MaximumDepthExceeded,
    /// [`parse_without_text_capture`] received a query that requests text.
    TextCaptureRequired,
    /// The HTML, or the text saved from it, is 4 GiB or longer: the
    /// [`Store`] indexes it with 32-bit offsets.
    InputTooLarge,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::EmptyQueries => write!(f, "parse requires at least one query"),
            ParseError::MaximumDepthExceeded => {
                write!(f, "HTML nesting depth exceeds the maximum supported depth")
            }
            ParseError::TextCaptureRequired => write!(
                f,
                "parse_without_text_capture cannot run queries that capture raw or normalized text; use parse"
            ),
            ParseError::InputTooLarge => {
                write!(f, "HTML input must be shorter than 4 GiB")
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse an HTML string against one or more pre-built [`Query`] objects and
/// return a [`Result`] containing a [`Store`] with all matched elements.
///
/// This is the main entry point of scah. It compiles the queries into one
/// [`Program`], streams the HTML through the [`XHtmlParser`], and returns the
/// result [`Store`].
///
/// # Errors
///
/// Returns [`ParseError::EmptyQueries`] for an empty query slice,
/// [`ParseError::MaximumDepthExceeded`] when nesting exceeds the supported
/// depth, and [`ParseError::InputTooLarge`] for HTML of 4 GiB or more.
///
/// # Parameters
///
/// - `html`: The HTML source string. All returned string slices in the
///   resulting [`Store`] borrow directly from this string (zero-copy).
/// - `queries`: A slice of compiled [`Query`] objects. Each query is
///   executed concurrently against the same token stream in a single pass.
///
/// # Example
///
/// ```rust
/// use scah::{Query, Save, parse};
///
/// let html = "<div><a href='link'>Hello</a></div>";
/// let queries = &[Query::all("a", Save::all())
///     .expect("valid selector")
///     .build()];
/// let store = parse(html, queries).expect("parse succeeds");
///
/// let links: Vec<_> = store.get("a").unwrap().collect();
/// assert_eq!(links.len(), 1);
/// assert_eq!(links[0].name(), "a");
/// ```
pub fn parse<'a: 'query, 'html: 'query, 'query: 'html, Q>(
    html: &'html str,
    queries: &'a [Q],
) -> Result<Store<'html, 'query>, ParseError>
where
    Q: QuerySpec<'query>,
{
    run(html, Cow::Owned(Program::compile(queries)), true)
}

/// Parse with queries compiled once by [`Program::compile`].
///
/// Compiling turns the queries into the flat tables the matcher runs. Reuse
/// one program to parse many documents with the same queries without paying
/// for that work on every call.
///
/// # Errors
///
/// The same as [`parse`].
///
/// # Example
///
/// ```rust
/// use scah::{Program, Query, Save, parse_compiled};
///
/// let queries = [Query::all("a[href]", Save::none())
///     .expect("valid selector")
///     .build()];
/// let program = Program::compile(&queries);
///
/// for html in ["<a href='/1'>1</a>", "<p><a href='/2'>2</a></p>"] {
///     let store = parse_compiled(html, &program).expect("parse succeeds");
///     assert_eq!(store.get("a[href]").unwrap().count(), 1);
/// }
/// ```
pub fn parse_compiled<'html, 'query: 'html>(
    html: &'html str,
    program: &'query Program<'query>,
) -> Result<Store<'html, 'query>, ParseError> {
    run(html, Cow::Borrowed(program), true)
}

/// Parse queries that do not request raw or normalized text.
///
/// This entry point references only the no-capture parser specialization, so
/// link-time optimization can discard entity decoding and text normalization.
pub fn parse_without_text_capture<'a: 'query, 'html: 'query, 'query: 'html, Q>(
    html: &'html str,
    queries: &'a [Q],
) -> Result<Store<'html, 'query>, ParseError>
where
    Q: QuerySpec<'query>,
{
    run(html, Cow::Owned(Program::compile(queries)), false)
}

/// [`parse_without_text_capture`] with a compiled [`Program`].
pub fn parse_compiled_without_text_capture<'html, 'query: 'html>(
    html: &'html str,
    program: &'query Program<'query>,
) -> Result<Store<'html, 'query>, ParseError> {
    run(html, Cow::Borrowed(program), false)
}

fn run<'html, 'query: 'html>(
    html: &'html str,
    program: Cow<'query, Program<'query>>,
    capture: bool,
) -> Result<Store<'html, 'query>, ParseError> {
    if program.root_sections().is_empty() {
        return Err(ParseError::EmptyQueries);
    }
    if !capture && program.features().text.any() {
        return Err(ParseError::TextCaptureRequired);
    }
    if html.len() >= u32::MAX as usize {
        return Err(ParseError::InputTooLarge);
    }
    let query_count = program.root_sections().len();
    // Queries that can stop early skip reserving storage for the whole document.
    let capacity = (!program.features().all_roots_first).then_some(html.len());
    let mut parser = XHtmlParser::from_program(program, capacity, None);
    let mut reader = Reader::new(html);
    parser.trace_parse_started(html.len(), query_count);
    if capture {
        parser.run(&mut reader);
    } else {
        parser.run_without_text_capture(&mut reader);
    }

    if let Some(err) = parser.take_parse_error() {
        return Err(err);
    }
    let store = parser.finish();
    if store.overflowed() {
        return Err(ParseError::InputTooLarge);
    }
    Ok(store)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_slice_returns_error() {
        let html = "<main><a href='x'>x</a></main>";
        let queries: &[Query] = &[];

        let result = parse(html, queries);

        assert!(matches!(result, Err(ParseError::EmptyQueries)));
    }

    #[test]
    fn non_empty_query_slice_succeeds() {
        let html = "<main><a href='x'>x</a></main>";
        let queries = &[Query::all("a", Save::all())
            .expect("valid selector")
            .build()];

        let store = parse(html, queries).expect("parse succeeds");

        assert_eq!(store.get("a").unwrap().count(), 1);
    }

    #[test]
    fn parse_first_query_skips_full_document_preallocation() {
        let filler = "<span class=\"filler\"></span>".repeat(10_000);
        let html_len = filler.len();
        let html = format!("<div id=\"hit\"></div>{}", filler);

        let query = Query::first("#hit", Save::none()).unwrap().build();
        let queries = &[query];
        let store = parse(&html, queries).unwrap();

        // Early-exit path uses XHtmlParser::new → Store::default()
        // which creates empty arenas. After parsing one match, capacity
        // should be driven by Vec growth (~4-8), not the full document
        // length (capacity path would reserve html_len / 48 ≈ 5k+).
        let capacity_path_reservation = html_len / 48;
        assert!(
            store.row_capacity() < capacity_path_reservation,
            "early-exit parse capacity ({}) must be far below full-document reservation ({})",
            store.row_capacity(),
            capacity_path_reservation,
        );
        assert!(
            store.attributes.capacity() < html_len / 24,
            "early-exit parse must not preallocate attribute arena"
        );

        // Results must still be correct.
        let hits: Vec<_> = store.get("#hit").unwrap().collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name(), "div");
    }

    #[test]
    fn parse_all_query_uses_capacity_preallocation_path() {
        let html = format!(
            "<div id=\"hit\"></div>{}",
            "<span class=\"filler\"></span>".repeat(5_000)
        );

        let query = Query::all("#hit", Save::none()).unwrap().build();
        let queries = &[query];
        let store = parse(&html, queries).unwrap();

        // .all() queries don't have exit_at_section_end → uses capacity path.
        assert!(
            store.row_capacity() > 0,
            "non-early-exit parse must preallocate element arena"
        );

        // Text buffers should NOT be reserved when Save::none().
        assert_eq!(
            store.text.raw_text.capacity(),
            0,
            "capacity path with Save::none must skip raw-text buffer preallocation"
        );
        assert_eq!(
            store.text.text.capacity(),
            0,
            "capacity path with Save::none must skip normalized-text buffer preallocation"
        );
    }

    #[test]
    fn parse_name_only_query_skips_attribute_preallocation() {
        let html = "<div data-value='x'></div>".repeat(5_000);
        let query = Query::all("div[data-value]", Save::name_only())
            .unwrap()
            .build();
        let queries = &[query];

        let store = parse(&html, queries).unwrap();

        assert_eq!(store.get("div[data-value]").unwrap().count(), 5_000);
        assert_eq!(store.attributes.capacity(), 0);
    }

    #[test]
    fn parse_with_save_text_reserves_text_buffer() {
        let html = "<div>text content here</div>".repeat(5_000);

        let query = Query::all("div", Save::only_text()).unwrap().build();
        let queries = &[query];
        let store = parse(&html, queries).unwrap();

        // Normalized text should be preallocated when saving text.
        assert!(
            store.text.text.capacity() > 0,
            "normalized-text buffer must be preallocated when queries need text"
        );
    }

    #[test]
    fn parse_first_early_exit_still_captures_text_when_needed() {
        let html = "<div id=\"hit\">important text</div>".to_string()
            + &"<span>filler</span>".repeat(1_000);

        let query = Query::first("#hit", Save::only_text()).unwrap().build();
        let queries = &[query];
        let store = parse(&html, queries).unwrap();

        let hits: Vec<_> = store.get("#hit").unwrap().collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text(), Some("important text"));

        // Early-exit with XHtmlParser::new uses Store::default() with no
        // preallocation, but matches are still recorded correctly.
    }

    #[test]
    fn element_attribute_lookup_is_case_insensitive_for_html_attributes() {
        let html = "<a HREF='x'></a>";
        let queries = &[Query::all("a", Save::all())
            .expect("valid selector")
            .build()];
        let store = parse(html, queries).expect("parse succeeds");
        let a = store.get("a").unwrap().next().unwrap();

        assert_eq!(a.attribute("href"), Some("x"));
        assert_eq!(a.attribute("HREF"), Some("x"));
        assert_eq!(a.attribute("Href"), Some("x"));
    }

    #[test]
    fn name_only_capture_matches_attributes_without_storing_them() {
        let html = concat!(
            "<a id='one' class='promoted' href='/one'>one</a>",
            "<a id='two' class='promoted' href='/two'>two</a>",
            "<a id='three' class='promoted' href='/three'>three</a>"
        );
        let queries = &[Query::all("a.promoted[href]", Save::name_only())
            .unwrap()
            .build()];

        let store = parse(html, queries).unwrap();
        let anchors = store.get("a.promoted[href]").unwrap().collect::<Vec<_>>();
        assert_eq!(anchors.len(), 3);
        let anchor = anchors[0];
        assert_eq!(anchor.name(), "a");
        assert_eq!(anchor.id(), None);
        assert_eq!(anchor.class(), None);
        assert_eq!(anchor.attributes(), None);
        assert_eq!(store.attributes.len(), 0);
    }

    #[test]
    fn name_only_queries_discard_attributes_when_another_query_saves() {
        let html = "<a href='/kept'>link</a>";
        let queries = &[
            Query::all("a[href='/missing']", Save::name_only())
                .unwrap()
                .build(),
            Query::all("a", Save::name_only()).unwrap().build(),
        ];

        let store = parse(html, queries).unwrap();
        assert_eq!(store.get("a").unwrap().count(), 1);
        assert_eq!(store.attributes.len(), 0);
    }

    #[test]
    fn attribute_capture_is_independent_for_queries_on_the_same_element() {
        let html = "<a id='hero' class='promoted' href='/kept'>link</a>";
        let queries = &[
            Query::all("a.promoted[href]", Save::name_only())
                .unwrap()
                .build(),
            Query::all("a", Save::none()).unwrap().build(),
        ];

        let store = parse(html, queries).unwrap();
        let lean = store.get("a.promoted[href]").unwrap().next().unwrap();
        let complete = store.get("a").unwrap().next().unwrap();
        assert_eq!(lean.attributes(), None);
        assert_eq!(complete.id(), Some("hero"));
        assert_eq!(complete.class(), Some("promoted"));
        assert_eq!(complete.attribute("href"), Some("/kept"));
    }
}
