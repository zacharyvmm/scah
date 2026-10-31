//! Owned query and store handles for scah's foreign-language bindings.
//!
//! [`scah::Store`] borrows the HTML it parsed and the selector strings of its
//! queries. Python, Node and C callers cannot express those borrows, so this
//! crate pairs each borrowing value with the allocation it borrows from:
//!
//! - [`OwnedQuery`] keeps a compiled [`Query`] together with its selector
//!   strings.
//! - [`OwnedStore`] keeps a parsed [`Store`] together with its HTML and the
//!   selector strings of the queries it ran.
//!
//! Both are immutable, `Send` and `Sync`, and hand out borrows that last no
//! longer than `self`.
//!
//! The [`arrow`] module exports query results as Apache Arrow columns through
//! the Arrow C Data Interface, borrowing strings from the store instead of
//! copying them.
//!
//! The [`utf16`] module encodes UTF-16 strings, such as Java's and .NET's, as
//! the UTF-8 HTML an [`OwnedStore`] parses, in one pass.

pub mod arrow;
pub mod utf16;

use scah::lazy::LazyQueryBuilder;
use scah::{ParseError, Query, SelectorParseError, Store};
use std::sync::Arc;

/// A compiled query that owns its selector strings.
#[derive(Clone)]
pub struct OwnedQuery {
    // Borrows `tape`, so it is declared first and dropped first.
    query: Query<'static>,
    /// Every selector string of the query, in one shared allocation that is
    /// never mutated.
    tape: Arc<Vec<u8>>,
}

impl OwnedQuery {
    /// Compile `builder`, reporting the first invalid selector.
    pub fn build(builder: LazyQueryBuilder<String>) -> Result<Self, SelectorParseError> {
        // SAFETY: the query borrows the tape, which `Self` keeps alive and
        // never mutates for as long as the query lives.
        let (tape, query) = unsafe { builder.try_to_query() }?;
        Ok(Self { query, tape })
    }

    /// The compiled query, borrowed for no longer than `self`.
    pub fn query(&self) -> &Query<'_> {
        &self.query
    }
}

impl std::fmt::Debug for OwnedQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.query.fmt(f)
    }
}

/// A parsed [`Store`] that owns its HTML and its queries' selector strings.
pub struct OwnedStore {
    // Borrows `_tapes` and `_html`, so it is declared first and dropped first.
    store: Store<'static, 'static>,
    _tapes: Box<[Arc<Vec<u8>>]>,
    _html: String,
}

impl OwnedStore {
    /// Parse `html` against `queries`.
    ///
    /// The HTML is moved in, not copied. The store shares each query's
    /// selector strings, so the queries may be dropped once this returns.
    pub fn parse(html: String, queries: &[&OwnedQuery]) -> Result<Self, ParseError> {
        let compiled: Vec<&Query<'static>> = queries.iter().map(|q| &q.query).collect();
        // SAFETY: the store keeps slices of `html` and of the query tapes,
        // which `Self` owns and never mutates. Of the queries it keeps only
        // their selector strings, which point into the tapes, so `compiled`
        // and the queries themselves only have to outlive this call.
        let store = unsafe {
            let html: &'static str = std::mem::transmute::<&str, _>(html.as_str());
            let compiled: &'static [&'static Query<'static>] =
                std::mem::transmute::<&[&Query<'static>], _>(compiled.as_slice());
            scah::parse(html, compiled)?
        };
        Ok(Self {
            store,
            _tapes: queries.iter().map(|q| Arc::clone(&q.tape)).collect(),
            _html: html,
        })
    }

    /// The parsed store, borrowed for no longer than `self`.
    pub fn store(&self) -> &Store<'_, '_> {
        &self.store
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scah::Save;
    use scah::lazy::LazyQuery;

    fn query(builder: LazyQueryBuilder<String>) -> OwnedQuery {
        OwnedQuery::build(builder).unwrap()
    }

    #[test]
    fn store_outlives_caller_strings() {
        let selector = String::from("a");
        let q = query(LazyQuery::all(selector.clone(), Save::all()));
        drop(selector);

        let store = OwnedStore::parse("<div><a href='x'>hi</a></div>".into(), &[&q]).unwrap();
        let a = store.store().get("a").unwrap().next().unwrap();
        assert_eq!(a.name(), "a");
        assert_eq!(a.attribute("href"), Some("x"));
        assert_eq!(a.text(), Some("hi"));
    }

    #[test]
    fn store_outlives_queries() {
        let q = query(LazyQuery::first("p".to_owned(), Save::only_text()));
        let store = OwnedStore::parse("<p>text</p>".into(), &[&q]).unwrap();
        drop(q);
        let p = store.store().get("p").unwrap().next().unwrap();
        assert_eq!(p.text(), Some("text"));
    }

    #[test]
    fn parses_several_queries() {
        let a = query(LazyQuery::all("a".to_owned(), Save::none()));
        let nested = query(
            LazyQuery::all("ul".to_owned(), Save::none())
                .then(|ul| [ul.all("li".to_owned(), Save::only_text())]),
        );
        let html = "<a></a><ul><li>1</li><li>2</li></ul><a></a>";
        let store = OwnedStore::parse(html.into(), &[&a, &nested]).unwrap();
        drop((a, nested));

        assert_eq!(store.store().get("a").unwrap().count(), 2);
        let ul = store.store().get("ul").unwrap().next().unwrap();
        let items: Vec<_> = ul.get("li").unwrap().map(|li| li.text().unwrap()).collect();
        assert_eq!(items, ["1", "2"]);
    }

    #[test]
    fn shares_a_query_across_stores() {
        let q = query(LazyQuery::all("b".to_owned(), Save::only_text()));
        let one = OwnedStore::parse("<b>1</b>".into(), &[&q]).unwrap();
        let two = OwnedStore::parse("<b>2</b><b>3</b>".into(), &[&q, &q]).unwrap();
        drop(q);

        assert_eq!(one.store().get("b").unwrap().count(), 1);
        let texts: Vec<_> = two.store().query(1).unwrap().map(|b| b.text()).collect();
        assert_eq!(texts, [Some("2"), Some("3")]);
    }

    #[test]
    fn rejects_empty_queries() {
        assert!(matches!(
            OwnedStore::parse("<a></a>".into(), &[]),
            Err(ParseError::EmptyQueries)
        ));
    }

    #[test]
    fn reports_invalid_selectors_at_build() {
        assert!(OwnedQuery::build(LazyQuery::all("a[".to_owned(), Save::none())).is_err());
    }

    #[test]
    fn handles_are_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<OwnedQuery>();
        assert_send_sync::<OwnedStore>();
    }
}
