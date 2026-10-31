//! # scah: CSS selectors over streaming HTML
//!
//! **scah** (*scan HTML*) extracts the elements you select from an HTML
//! document in a single forward pass, without building a DOM. You describe
//! what you want with CSS selectors; scah matches them while the document
//! streams through its parser and keeps only the matches.
//!
//! It sits between a SAX-style tokenizer, where you track parser state by
//! hand, and a DOM, where you pay to build a whole tree before you can query
//! it.
//!
//! ## Quick start
//!
//! ```rust
//! use scah::{Query, Save, parse};
//!
//! let html = r#"<ul><li><a href="/one">One</a></li><li><a href="/two">Two</a></li></ul>"#;
//!
//! let queries = [Query::all("a[href]", Save::only_text())?.build()];
//! let store = parse(html, &queries)?;
//!
//! let links: Vec<_> = store
//!     .get("a[href]")
//!     .unwrap()
//!     .map(|a| (a.text().unwrap(), a.attribute("href").unwrap()))
//!     .collect();
//! assert_eq!(links, [("One", "/one"), ("Two", "/two")]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Concepts
//!
//! - **[`Query`]**: a selector and what to save for its matches.
//!   [`Query::all`] keeps every match; [`Query::first`] keeps the first and
//!   lets the parse stop early. Selectors are validated when the query is
//!   built, and anything scah cannot evaluate in one forward pass is a
//!   [`SelectorParseError`], never a silently narrower match.
//! - **[`Save`]**: which content to keep for each match: inner HTML,
//!   source-preserving raw text, normalized text, attributes. Saving less is
//!   faster, so ask only for what you read.
//! - **[`parse`]**: runs every query in one pass and returns a [`Store`].
//! - **[`Store`]** and **[`ElementRef`]**: the results. Look them up by
//!   selector with [`Store::get`], or by position with [`Store::query`] when
//!   two queries share a selector.
//!
//! ## Nested queries
//!
//! [`QueryBuilder::then`] scopes child queries to each match of their parent,
//! so related data stays together and the child selectors only run inside
//! the elements you care about:
//!
//! ```rust
//! use scah::{Query, Save, parse};
//!
//! let html = r#"
//!     <div class="product"><h2>Lamp</h2><span class="price">$20</span></div>
//!     <div class="product"><h2>Desk</h2><span class="price">$150</span></div>
//! "#;
//!
//! let queries = [Query::all("div.product", Save::none())?
//!     .then(|product| {
//!         Ok([
//!             product.first("> h2", Save::only_text())?,
//!             product.first(".price", Save::only_text())?,
//!         ])
//!     })?
//!     .build()];
//! let store = parse(html, &queries)?;
//!
//! for product in store.get("div.product").unwrap() {
//!     let name = product.get("> h2").unwrap().next().unwrap();
//!     let price = product.get(".price").unwrap().next().unwrap();
//!     println!("{}: {}", name.text().unwrap(), price.text().unwrap());
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Compile-time queries
//!
//! For selectors known at compile time, [`query!`] validates them while your
//! crate compiles and builds the query without allocating.
//!
//! ## Parsing many documents
//!
//! [`parse`] compiles its queries on every call. To run the same queries over
//! many documents, compile them once with [`Program::compile`] and call
//! [`parse_compiled`]. When no query saves text,
//! [`parse_without_text_capture`] lets the linker drop the text-extraction
//! code.
//!
//! ## What borrows from what
//!
//! Element names, ids, classes, attributes, and inner HTML are `&str` slices
//! of the input HTML. Raw and normalized text are assembled while parsing and
//! borrow from the [`Store`].
//!
//! ## Guides
//!
//! - [Selector support](https://github.com/zacharyvmm/scah/blob/main/docs/selectors.md):
//!   every supported selector, and why some are rejected.
//! - [Text extraction](https://github.com/zacharyvmm/scah/blob/main/docs/text-extraction.md):
//!   what `inner_html`, `raw_text`, and `text` contain.
//! - [Architecture](https://github.com/zacharyvmm/scah/blob/main/docs/architecture.md):
//!   how a parse works, and which crate does what.

use std::borrow::Cow;

mod capture;
pub mod debug;
mod engine;
mod output;
mod parser;
mod support;

#[cfg(all(any(debug_assertions, test), feature = "otel"))]
mod otel;

pub use parser::XHtmlParser;
pub use scah_html::{Attribute, Reader, XHtmlElement};
/// Build a query at compile time.
///
/// `query!` parses and validates its selectors while your crate compiles, so
/// an unsupported selector is a compile error rather than a runtime
/// [`SelectorParseError`](https://docs.rs/scah/latest/scah/struct.SelectorParseError.html).
/// It expands to a `StaticQuery` backed by inline arrays, with no heap
/// allocation.
///
/// Each node is `all(selector, save)` or `first(selector, save)`, optionally
/// followed by `=> { child, child, ... }` for queries scoped to its matches,
/// like `QueryBuilder::then`. Selectors must be string literals. `save` must
/// be a `Save` constructor that takes no arguments, such as `Save::all()` or
/// `Save::only_text()`, optionally followed by `.without_attributes()`.
///
/// ```rust
/// use scah::{parse, query};
///
/// let html = "<article><h1>Title</h1><a href='/one'>One</a></article>";
///
/// let queries = [query! {
///     all("article", Save::none()) => {
///         first("> h1", Save::only_text()),
///         all("a[href]", Save::none()),
///     }
/// }];
///
/// let store = parse(html, &queries).expect("parse succeeds");
/// let article = store.get("article").unwrap().next().unwrap();
/// assert_eq!(article.get("> h1").unwrap().next().unwrap().text(), Some("Title"));
/// ```
///
/// Selectors that scah cannot evaluate fail to compile:
///
/// ```compile_fail
/// use scah::query;
///
/// let query = query! { all("li:last-child", Save::none()) };
/// ```
pub use scah_macros::query;
#[doc(hidden)]
pub use scah_query_ir::lazy;
pub use scah_query_ir::{
    AnPlusB, AttributeCaseSensitivity, AttributeSelection, AttributeSelectionKind,
    AttributeSelections, ClassSelections, Combinator, ElementPredicate, IElement,
    LocalLogicalPredicate, LocalSelectorList, LogicalPredicates, MAX_SELECTOR_NESTING_DEPTH,
    Program, ProgramFeatures, Query, QueryBuilder, QueryFactory, QuerySection, QuerySectionId,
    QuerySpec, Save, SelectionKind, SelectorParseError, StaticQuery, StructuralMatchContext,
    StructuralPredicate, StructuralPredicates, TextRequirements, Transition, TransitionId,
};
pub use scah_store::{
    Attributes, CapacityOptions, ElementId, ElementRef, Elements, HeapUsage, Store,
};

// Runs the Rust examples in the README as doctests, so they stay correct.
#[cfg(doctest)]
#[doc = include_str!("../../../README.md")]
struct ReadmeDoctests;

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
    pub use scah_html::{ScopeKind, TagFlags};

    #[cfg(feature = "bench-internals")]
    pub use crate::capture::TextPathStats;
    #[cfg(feature = "simd-bench-internals")]
    use crate::{ParseError, Program, QuerySpec, Store};
    #[cfg(feature = "simd-bench-internals")]
    use scah_html::BlockClassifier;
    #[cfg(feature = "simd-bench-internals")]
    use scah_html::IndexingMode;

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

    /// The tag indexer [`parse_with_indexing_mode`] uses.
    #[cfg(feature = "simd-bench-internals")]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum IndexingStrategy {
        /// The production choice, as [`parse`](crate::parse) makes it.
        Policy,
        /// Search for each tag as the parse reaches it.
        Rolling,
        /// Index the whole document ahead with the structural tape.
        Tape,
    }

    /// Parse HTML with a fixed indexer strategy for strategy crossover
    /// benchmarks. Apart from the indexer, it parses exactly like
    /// [`parse`](crate::parse): same limits, result reservation and trace.
    #[cfg(feature = "simd-bench-internals")]
    pub fn parse_with_indexing_mode<'a: 'query, 'html: 'query, 'query: 'html, Q>(
        html: &'html str,
        queries: &'a [Q],
        strategy: IndexingStrategy,
    ) -> Result<Store<'html, 'query>, ParseError>
    where
        Q: QuerySpec<'query>,
    {
        let indexing_mode = match strategy {
            IndexingStrategy::Policy => None,
            IndexingStrategy::Rolling => Some(IndexingMode::Rolling),
            IndexingStrategy::Tape => Some(IndexingMode::ForcedTape),
        };
        crate::run(
            html,
            std::borrow::Cow::Owned(Program::compile(queries)),
            true,
            indexing_mode,
        )
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

impl From<scah_html::DepthExceeded> for ParseError {
    fn from(_: scah_html::DepthExceeded) -> Self {
        ParseError::MaximumDepthExceeded
    }
}

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
/// - `queries`: The queries to run, built with [`Query`] or [`query!`]. All
///   of them run together in a single pass over the document.
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
    run(html, Cow::Owned(Program::compile(queries)), true, None)
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
    run(html, Cow::Borrowed(program), true, None)
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
    run(html, Cow::Owned(Program::compile(queries)), false, None)
}

/// [`parse_without_text_capture`] with a compiled [`Program`].
pub fn parse_compiled_without_text_capture<'html, 'query: 'html>(
    html: &'html str,
    program: &'query Program<'query>,
) -> Result<Store<'html, 'query>, ParseError> {
    run(html, Cow::Borrowed(program), false, None)
}

/// Parse `html` with a compiled program. `capture` is false for the
/// `*_without_text_capture` entry points, which reject text-saving queries.
fn run<'html, 'query: 'html>(
    html: &'html str,
    program: Cow<'query, Program<'query>>,
    capture: bool,
    indexing_mode: Option<scah_html::IndexingMode>,
) -> Result<Store<'html, 'query>, ParseError> {
    let parser = parse_document(html, program, capture, indexing_mode)?;
    checked(parser.finish())
}

/// [`run`], also returning the trace of the parse.
#[cfg(any(debug_assertions, test))]
fn run_traced<'html, 'query: 'html>(
    html: &'html str,
    program: Cow<'query, Program<'query>>,
    capture: bool,
    indexing_mode: Option<scah_html::IndexingMode>,
) -> Result<(Store<'html, 'query>, debug::TraceStore<'html, 'query>), ParseError> {
    let parser = parse_document(html, program, capture, indexing_mode)?;
    let (store, trace) = parser.finish_traced();
    Ok((checked(store)?, trace))
}

/// A parser that read all of `html`, ready to finish.
fn parse_document<'html, 'query: 'html>(
    html: &'html str,
    program: Cow<'query, Program<'query>>,
    capture: bool,
    indexing_mode: Option<scah_html::IndexingMode>,
) -> Result<XHtmlParser<'html, 'query>, ParseError> {
    check_limits(html, &program, capture)?;
    let query_count = program.root_sections().len();
    // Parses that can stop early do not reserve result rows for the whole input.
    let capacity = (!program.features().all_roots_first).then_some(html.len());
    let mut parser = XHtmlParser::from_program(program, capacity, indexing_mode);
    let mut reader = Reader::new(html);
    parser.trace_parse_started(html.len(), query_count);
    parser.run(&mut reader);

    match parser.take_parse_error() {
        Some(err) => Err(err),
        None => Ok(parser),
    }
}

/// `store`, unless a text range or an attribute index did not fit in it.
fn checked<'html, 'query: 'html>(
    store: Store<'html, 'query>,
) -> Result<Store<'html, 'query>, ParseError> {
    if store.overflowed() {
        return Err(ParseError::InputTooLarge);
    }
    Ok(store)
}

fn check_limits(html: &str, program: &Program<'_>, capture: bool) -> Result<(), ParseError> {
    if program.root_sections().is_empty() {
        return Err(ParseError::EmptyQueries);
    }
    if !capture && program.features().text.any() {
        return Err(ParseError::TextCaptureRequired);
    }
    if html.len() >= u32::MAX as usize {
        return Err(ParseError::InputTooLarge);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "simd-bench-internals")]
    fn indexing_mode_helper_traces_like_parse() {
        use scah_html::IndexingMode;

        let html = "<main><a href='x'>x</a><a>y</a></main>";
        let queries = &[Query::all("a", Save::all()).unwrap().build()];
        let events = |trace: &debug::TraceStore<'_, '_>| format!("{:?}", trace.events());

        let (_, expected) = debug::parse_traced(html, queries).unwrap();
        // The indexing modes `parse_with_indexing_mode` chooses between.
        for mode in [
            None,
            Some(IndexingMode::Rolling),
            Some(IndexingMode::ForcedTape),
        ] {
            let program = Cow::Owned(Program::compile(queries));
            let (_, trace) = run_traced(html, program, true, mode).unwrap();
            assert_eq!(events(&trace), events(&expected), "{mode:?}");
        }
    }

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
            store.attribute_count() < html_len / 24
                && store.heap_usage().attributes < html_len / 24 * 16,
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
            store.text_tapes().raw_text.capacity(),
            0,
            "capacity path with Save::none must skip raw-text buffer preallocation"
        );
        assert_eq!(
            store.text_tapes().text.capacity(),
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
        assert_eq!(store.heap_usage().attributes, 0);
    }

    #[test]
    fn parse_with_save_text_reserves_text_buffer() {
        let html = "<div>text content here</div>".repeat(5_000);

        let query = Query::all("div", Save::only_text()).unwrap().build();
        let queries = &[query];
        let store = parse(&html, queries).unwrap();

        // Normalized text should be preallocated when saving text.
        assert!(
            store.text_tapes().text.capacity() > 0,
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
        assert!(anchor.attributes().is_none());
        assert_eq!(store.attribute_count(), 0);
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
        assert_eq!(store.attribute_count(), 0);
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
        assert!(lean.attributes().is_none());
        assert_eq!(complete.id(), Some("hero"));
        assert_eq!(complete.class(), Some("promoted"));
        assert_eq!(complete.attribute("href"), Some("/kept"));
    }
}
