//! Zero-copy export of query results through the Apache Arrow C Data
//! Interface.
//!
//! An [`ArrowTable`] lays out the elements matched by one selector as
//! columns. The store already keeps every string as a span of one of three
//! buffers (the HTML and the two text tapes), and which one depends only on
//! the column, so each string column is a `Utf8View` array over a single
//! buffer of the store's: strings of up to 12 bytes are inlined in their
//! 16-byte view and longer ones point into that buffer. Building a table
//! copies no string data, and every exported struct keeps the table, and
//! through it the store, alive until the consumer releases it.
//!
//! ```text
//! mod.rs      ArrowTable::build: pick the rows, then one column per field
//! column.rs   the columns: uint32 ids and Utf8View strings
//! c_data.rs   the C Data and C Stream Interface structs and their release
//! ```

mod c_data;
mod column;
#[cfg(test)]
mod tests;

pub use c_data::{ARROW_FLAG_NULLABLE, ArrowArray, ArrowArrayStream, ArrowSchema};

use crate::OwnedStore;
use column::{Column, ColumnData};
use scah::{ElementId, Field, Store};
use std::ffi::CString;
use std::fmt;
use std::sync::Arc;

/// Why a table could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrowError {
    /// A requested attribute repeats a column name.
    DuplicateColumn(String),
    /// A requested attribute name contains a NUL byte.
    InvalidColumnName(String),
    /// A buffer does not fit Arrow's 32-bit view offsets.
    TooLarge(String),
}

impl fmt::Display for ArrowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateColumn(name) => write!(f, "duplicate column name `{name}`"),
            Self::InvalidColumnName(name) => {
                write!(f, "column name {name:?} contains a NUL byte")
            }
            Self::TooLarge(detail) => write!(f, "too large for an Arrow export: {detail}"),
        }
    }
}

impl std::error::Error for ArrowError {}

/// Query results laid out as Arrow columns.
///
/// Columns, in order:
///
/// | Column | Type | Nulls |
/// |--------|------|-------|
/// | `index` | `uint32` | never; the element's id in its store |
/// | `parent` | `uint32` | never; only with a parent selector |
/// | `tag` | `Utf8View` | never |
/// | `inner_html`, `raw_text`, `text` | `Utf8View` | when not saved |
/// | one per requested attribute | `Utf8View` | when absent or valueless |
pub struct ArrowTable {
    store: Arc<OwnedStore>,
    num_rows: usize,
    columns: Vec<Column>,
}

/// The fixed string columns, in order, and the fields they read.
const STRING_COLUMNS: [(&str, Field<'static>); 4] = [
    ("tag", Field::Name),
    ("inner_html", Field::InnerHtml),
    ("raw_text", Field::RawText),
    ("text", Field::Text),
];

impl ArrowTable {
    /// Lay out the elements matched by top-level section `selector`, or, when
    /// `parent` is given, the elements matched by child section `selector`
    /// under every element matched by top-level section `parent`.
    ///
    /// `attributes` names extra columns, read like
    /// [`scah::ElementRef::attribute`] except that `id` and `class` read the
    /// element's id and class. A selector without matches yields an empty
    /// table with the full schema.
    pub fn build(
        store: Arc<OwnedStore>,
        selector: &str,
        parent: Option<&str>,
        attributes: &[&str],
    ) -> Result<Self, ArrowError> {
        let names = column_names(parent.is_some(), attributes)?;
        let doc = store.store();
        let rows = select_rows(doc, selector, parent);

        let mut data = vec![ColumnData::ids(&rows.ids)];
        data.extend(rows.parents.map(ColumnData::UInt32));
        let fields = STRING_COLUMNS
            .iter()
            .map(|&(_, field)| field)
            .chain(attributes.iter().map(|&name| Field::attribute(name)));
        for field in fields {
            // Only a tag is always there.
            let nullable = field != Field::Name;
            data.push(ColumnData::strings(doc, &rows.ids, field, nullable)?);
        }

        let columns = names
            .into_iter()
            .zip(data)
            .map(|(name, data)| Column { name, data })
            .collect();
        Ok(Self {
            num_rows: rows.ids.len(),
            columns,
            store,
        })
    }

    /// The number of rows.
    pub fn num_rows(&self) -> usize {
        self.num_rows
    }

    /// The column names, in order.
    pub fn column_names(&self) -> impl Iterator<Item = &str> {
        self.columns
            .iter()
            .map(|column| column.name.to_str().expect("column names are UTF-8"))
    }

    /// Export the schema: a struct (record batch) with one field per column.
    pub fn export_schema(&self) -> ArrowSchema {
        c_data::export_schema(&self.columns)
    }

    /// Export the data as one struct array of [`ArrowTable::num_rows`] rows.
    ///
    /// The array and each of its children hold a reference to the table.
    pub fn export_array(self: &Arc<Self>) -> ArrowArray {
        c_data::export_array(self)
    }

    /// Export the schema and the data together.
    pub fn export(self: &Arc<Self>) -> (ArrowSchema, ArrowArray) {
        (self.export_schema(), self.export_array())
    }

    /// Export a stream that yields the table as a single record batch.
    pub fn export_stream(self: &Arc<Self>) -> ArrowArrayStream {
        c_data::export_stream(self.clone())
    }
}

/// The rows of a table: element ids, and with a parent selector, the id of
/// each row's parent.
struct Rows {
    ids: Vec<ElementId>,
    parents: Option<Vec<u32>>,
}

/// The results of top-level section `selector`, or of its child section
/// under each result of top-level section `parent`.
fn select_rows(store: &Store<'_, '_>, selector: &str, parent: Option<&str>) -> Rows {
    let Some(parent) = parent else {
        let ids = store.results(selector).unwrap_or_default().to_vec();
        return Rows { ids, parents: None };
    };
    let mut ids = Vec::new();
    let mut parents = Vec::new();
    for &parent in store.results(parent).unwrap_or_default() {
        let children = store.child_results(parent, selector).unwrap_or_default();
        ids.extend_from_slice(children);
        // Store rows are numbered in `u32`.
        parents.extend(std::iter::repeat_n(parent.index() as u32, children.len()));
    }
    Rows {
        ids,
        parents: Some(parents),
    }
}

/// The table's column names, or an error when an attribute reuses one.
fn column_names(has_parent: bool, attributes: &[&str]) -> Result<Vec<CString>, ArrowError> {
    let mut names: Vec<&str> = vec!["index"];
    if has_parent {
        names.push("parent");
    }
    names.extend(STRING_COLUMNS.iter().map(|&(name, _)| name));
    for &attribute in attributes {
        if names.contains(&attribute) {
            return Err(ArrowError::DuplicateColumn(attribute.to_owned()));
        }
        names.push(attribute);
    }
    names
        .into_iter()
        .map(|name| CString::new(name).map_err(|_| ArrowError::InvalidColumnName(name.to_owned())))
        .collect()
}
