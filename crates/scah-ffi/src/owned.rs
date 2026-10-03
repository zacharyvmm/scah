//! Queries and stores that own the data they borrow.

use scah::lazy::LazyQueryBuilder;
use scah::{ParseError, Query, SelectorParseError, Store};
use std::sync::Arc;

/// A compiled query that owns its selector strings.
///
/// Selector strings live in one shared tape allocation. The compiled [`Query`]
/// borrows that tape, so both travel together and the tape is never mutated.
#[derive(Clone)]
pub struct OwnedQuery {
    // Declared before `tape` so the borrowing query drops first.
    query: Query<'static>,
    tape: Arc<Vec<u8>>,
}

impl OwnedQuery {
    /// Compile `builder`, reporting the first invalid selector.
    pub fn build(builder: LazyQueryBuilder<String>) -> Result<Self, SelectorParseError> {
        // SAFETY: the returned query borrows the returned tape, which this
        // struct keeps alive and never mutates for as long as the query lives.
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
    // Declared first so the borrowing store drops before its backing data.
    store: Store<'static, 'static>,
    _tapes: Vec<Arc<Vec<u8>>>,
    html: String,
}

impl OwnedStore {
    /// Parse `html` against `queries`.
    ///
    /// The HTML string is moved, not copied. The store shares each query's
    /// selector tape, so the queries may be dropped once this returns.
    pub fn parse(html: String, queries: &[&OwnedQuery]) -> Result<Self, ParseError> {
        // SAFETY: `html` moves into the returned struct and is never mutated,
        // so its heap buffer outlives every slice the store takes from it.
        let html_str: &'static str = unsafe { std::mem::transmute::<&str, _>(html.as_str()) };

        // The store keeps slices of the selector tapes but never a reference
        // to a `Query` itself, so each query only has to outlive the parse.
        let store = match queries {
            [] => return Err(ParseError::EmptyQueries),
            // The common single-query case parses the shared query in place.
            [query] => {
                // SAFETY: the query outlives this call, and the returned
                // struct keeps its tape alive (see above).
                let query: &'static Query<'static> =
                    unsafe { std::mem::transmute::<&Query<'_>, _>(query.query()) };
                scah::parse(html_str, std::slice::from_ref(query))?
            }
            queries => {
                let compiled: Vec<Query<'static>> =
                    queries.iter().map(|q| q.query.clone()).collect();
                // SAFETY: the local vector outlives this call, and the
                // returned struct keeps the tapes alive (see above).
                let compiled_slice: &'static [Query<'static>] =
                    unsafe { std::slice::from_raw_parts(compiled.as_ptr(), compiled.len()) };
                scah::parse(html_str, compiled_slice)?
            }
        };

        Ok(Self {
            store,
            _tapes: queries.iter().map(|q| q.tape.clone()).collect(),
            html,
        })
    }

    /// The parsed store, borrowed for no longer than `self`.
    pub fn store(&self) -> &Store<'_, '_> {
        &self.store
    }

    /// The HTML the store was parsed from.
    ///
    /// Element names, ids, classes, attribute values, and inner HTML borrow
    /// from this string.
    pub fn html(&self) -> &str {
        &self.html
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
        assert_eq!(a.name, "a");
        assert_eq!(a.attribute(store.store(), "href"), Some("x"));
        assert_eq!(a.text(store.store()), Some("hi"));
    }

    #[test]
    fn store_outlives_queries() {
        let q = query(LazyQuery::first("p".to_owned(), Save::only_text()));
        let store = OwnedStore::parse("<p>text</p>".into(), &[&q]).unwrap();
        drop(q);
        let p = store.store().get("p").unwrap().next().unwrap();
        assert_eq!(p.text(store.store()), Some("text"));
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
        let items: Vec<_> = ul
            .get(store.store(), "li")
            .unwrap()
            .map(|li| li.text(store.store()).unwrap())
            .collect();
        assert_eq!(items, ["1", "2"]);
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
