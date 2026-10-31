//! The polars expression plugin behind `scah.polars.parse`.
//!
//! Polars calls [`parse`] with a chunk of a string column of HTML documents
//! and the query, described by [`SectionSpec`]s. It runs the query on every
//! document and returns one value per document, shaped by the query:
//!
//! ```text
//! Query.all(...)    list[struct]   every match, in document order
//! Query.first(...)  struct         the first match, or null
//! ```
//!
//! Each struct holds the match's `tag`, the content its `Save` keeps
//! (`inner_html`, `raw_text`, `text`), its attributes, and one field per
//! child section, named by the child's selector and shaped the same way.
//! A null document gives a null value.
//!
//! The plugin is elementwise, so polars runs it on each chunk or morsel of
//! the column independently, without the GIL. The in-memory engine hands it
//! a whole column as one chunk, so a call running alone also spreads a
//! large chunk over threads of its own (see [`parse`]).
//!
//! The `_polars_plugin_*` functions at the bottom are the plugin ABI that
//! polars loads from this library.

use polars_arrow::array::{
    Array, LIST_VALUES_NAME, ListArray, MutableBinaryViewArray, StructArray,
};
use polars_arrow::bitmap::MutableBitmap;
use polars_arrow::datatypes::{ArrowDataType, Field as ArrowField};
use polars_arrow::ffi::{self, ArrowSchema};
use polars_arrow::offset::Offsets;
use polars_core::prelude::*;
use polars_ffi::version_0::{CallerContext, SeriesExport, export_series, import_series_buffer};
use pyo3::IntoPyObject;
use scah_core::lazy::{LazyQuery, LazyQueryBuilder};
use scah_core::{ElementRef, Elements, Program, QuerySectionId, Save, SelectionKind};
use scah_ffi::OwnedQuery;
use serde::Deserialize;
use std::cell::RefCell;
use std::ffi::{CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, OnceLock};

/// One query section, as `Query._sections()` hands it to Python and Python
/// hands it back to the plugin.
#[derive(Debug, Deserialize, IntoPyObject)]
pub(crate) struct SectionSpec {
    pub selector: String,
    /// `Query.first` rather than `Query.all`.
    pub first: bool,
    /// Index of the section this one is nested under; `None` for the root.
    pub parent: Option<usize>,
    pub inner_html: bool,
    pub raw_text: bool,
    pub text: bool,
    pub attributes: bool,
}

impl SectionSpec {
    pub(crate) fn new(
        selector: &str,
        kind: SelectionKind,
        parent: Option<QuerySectionId>,
        save: Save,
    ) -> Self {
        Self {
            selector: selector.to_owned(),
            first: kind == SelectionKind::First,
            parent: parent.map(|parent| parent.index()),
            inner_html: save.inner_html,
            raw_text: save.raw_text,
            text: save.text,
            attributes: save.attributes,
        }
    }

    fn save(&self) -> Save {
        Save {
            inner_html: self.inner_html,
            raw_text: self.raw_text,
            text: self.text,
            attributes: self.attributes,
        }
    }
}

#[derive(Deserialize)]
struct ParseKwargs {
    /// The query's sections, root first, each after its parent.
    sections: Vec<SectionSpec>,
    /// One string field per named attribute, or `None` for a single
    /// `attributes` field listing every attribute as a name/value pair.
    attributes: Option<Vec<String>>,
}

/// A section and its child sections, in nesting order.
struct Layout<'k> {
    spec: &'k SectionSpec,
    children: Vec<Layout<'k>>,
}

impl<'k> Layout<'k> {
    /// The section tree, checking that every parent comes before its
    /// children.
    fn new(sections: &'k [SectionSpec]) -> PolarsResult<Self> {
        let Some(root) = sections.first() else {
            polars_bail!(InvalidOperation: "scah.polars.parse needs a query with at least one section");
        };
        polars_ensure!(
            root.parent.is_none(),
            InvalidOperation: "the first query section must be the root"
        );
        for (index, section) in sections.iter().enumerate().skip(1) {
            polars_ensure!(
                section.parent.is_some_and(|parent| parent < index),
                InvalidOperation: "query section {index} must be nested under an earlier section"
            );
        }
        Ok(Self::subtree(sections, 0))
    }

    fn subtree(sections: &'k [SectionSpec], index: usize) -> Self {
        let children = (index + 1..sections.len())
            .filter(|&child| sections[child].parent == Some(index))
            .map(|child| Self::subtree(sections, child))
            .collect();
        Self {
            spec: &sections[index],
            children,
        }
    }

    /// The polars type of this section's values.
    fn dtype(&self, attributes: Option<&[String]>) -> PolarsResult<DataType> {
        let row = DataType::Struct(self.row_fields(attributes)?);
        Ok(if self.spec.first {
            row
        } else {
            DataType::List(Box::new(row))
        })
    }

    /// The fields of a match's struct.
    fn row_fields(&self, attributes: Option<&[String]>) -> PolarsResult<Vec<Field>> {
        let spec = self.spec;
        let mut fields = vec![Field::new("tag".into(), DataType::String)];
        for (saved, name) in [
            (spec.inner_html, "inner_html"),
            (spec.raw_text, "raw_text"),
            (spec.text, "text"),
        ] {
            if saved {
                fields.push(Field::new(name.into(), DataType::String));
            }
        }
        if spec.attributes {
            match attributes {
                Some(names) => fields.extend(
                    names
                        .iter()
                        .map(|name| Field::new(name.into(), DataType::String)),
                ),
                None => fields.push(Field::new("attributes".into(), pair_dtype())),
            }
        }
        for child in &self.children {
            let dtype = child.dtype(attributes)?;
            fields.push(Field::new(child.spec.selector.as_str().into(), dtype));
        }

        for (index, field) in fields.iter().enumerate() {
            if fields[..index].iter().any(|seen| seen.name == field.name) {
                polars_bail!(
                    Duplicate: "the matches of `{}` would have two fields named `{}`; \
                    rename the attribute or the child selector",
                    spec.selector, field.name
                );
            }
        }
        Ok(fields)
    }
}

/// The type of the `attributes` field: name/value pairs, in the order
/// class, id, then the rest as they appear in the tag.
fn pair_dtype() -> DataType {
    DataType::List(Box::new(DataType::Struct(vec![
        Field::new("name".into(), DataType::String),
        Field::new("value".into(), DataType::String),
    ])))
}

fn parse_output(input_fields: &[Field], kwargs: ParseKwargs) -> PolarsResult<Field> {
    let input = &input_fields[0];
    polars_ensure!(
        input.dtype == DataType::String,
        InvalidOperation: "scah.polars.parse needs a String column of HTML, got {}",
        input.dtype
    );
    let layout = Layout::new(&kwargs.sections)?;
    let dtype = layout.dtype(kwargs.attributes.as_deref())?;
    Ok(Field::new(input.name.clone(), dtype))
}

fn parse(inputs: &[Series], kwargs: ParseKwargs) -> PolarsResult<Series> {
    let html = inputs[0].str()?;
    let layout = Layout::new(&kwargs.sections)?;
    // Check field names before parsing anything.
    layout.dtype(kwargs.attributes.as_deref())?;

    let query = OwnedQuery::build(query_builder(&kwargs.sections))
        .map_err(|err| polars_err!(InvalidOperation: "invalid selector: {err}"))?;
    let queries = [query.query()];
    let program = Program::compile(&queries);

    let documents: Vec<Option<&str>> = html.iter().collect();
    let attributes = kwargs.attributes.as_deref();
    let run = |(first_row, documents): (usize, &[Option<&str>])| -> PolarsResult<Box<dyn Array>> {
        let mut column = Column::new(&layout, attributes);
        for (row, document) in (first_row..).zip(documents) {
            let Some(document) = document else {
                column.push_null();
                continue;
            };
            let store = scah_core::parse_compiled(document, &program)
                .map_err(|err| polars_err!(ComputeError: "row {row}: {err}"))?;
            column.push(store.query(0))?;
        }
        Ok(column.finish())
    };

    let total: usize = documents
        .iter()
        .flatten()
        .map(|document| document.len())
        .sum();
    // Polars runs the plugin on one of its threads per call, and its
    // streaming engine runs many calls at once. A call that starts while no
    // other is running splits its documents into runs of about
    // `MIN_BYTES_PER_RUN` and shares them with helper threads, which stop
    // taking runs once another call starts, so concurrent calls don't
    // oversubscribe the cores. Each run is a chunk of the output.
    let running = Running::enter();
    let runs = if running.alone {
        (total / MIN_BYTES_PER_RUN).clamp(1, *MAX_THREADS * 4)
    } else {
        1
    };
    let runs = split_by_bytes(&documents, runs, total);
    let chunks = if runs.len() == 1 {
        vec![run(runs[0])?]
    } else {
        let next = AtomicUsize::new(0);
        let results: Vec<OnceLock<PolarsResult<Box<dyn Array>>>> =
            runs.iter().map(|_| OnceLock::new()).collect();
        let work = |helper: bool| {
            while !helper || RUNNING.load(Ordering::Acquire) == 1 {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(&part) = runs.get(index) else { break };
                let _ = results[index].set(run(part));
            }
        };
        // A panicking helper makes the scope panic, which `report` catches.
        std::thread::scope(|scope| {
            for _ in 1..(*MAX_THREADS).min(runs.len()) {
                scope.spawn(|| work(true));
            }
            work(false);
        });
        results
            .into_iter()
            .map(|result| {
                result
                    .into_inner()
                    .expect("the caller runs what helpers leave")
            })
            .collect::<PolarsResult<Vec<_>>>()?
    };
    drop(running);
    Series::from_arrow_chunks(html.name().clone(), chunks)
}

/// The least HTML worth handing to another thread.
const MIN_BYTES_PER_RUN: usize = 1 << 20;

/// The most threads one call runs on: `POLARS_MAX_THREADS`, like polars'
/// own pool, or else one per core.
static MAX_THREADS: LazyLock<usize> = LazyLock::new(|| {
    std::env::var("POLARS_MAX_THREADS")
        .ok()
        .and_then(|threads| threads.parse().ok())
        .or_else(|| std::thread::available_parallelism().ok().map(Into::into))
        .unwrap_or(1usize)
        .max(1)
});

/// Calls of `parse` running now.
static RUNNING: AtomicUsize = AtomicUsize::new(0);

/// A running call of `parse`, counted in [`RUNNING`] until dropped.
struct Running {
    /// No other call was running when this one started.
    alone: bool,
}

impl Running {
    fn enter() -> Self {
        let alone = RUNNING.fetch_add(1, Ordering::AcqRel) == 0;
        Self { alone }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        RUNNING.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Split `documents` into at most `parts` runs of about the same number of
/// bytes, each with the index of its first row.
fn split_by_bytes<'a, 'h>(
    documents: &'a [Option<&'h str>],
    parts: usize,
    total: usize,
) -> Vec<(usize, &'a [Option<&'h str>])> {
    let mut runs = Vec::with_capacity(parts);
    let mut start = 0;
    let mut bytes = 0;
    for (row, document) in documents.iter().enumerate() {
        bytes += document.map_or(0, str::len);
        // Close the run once it reaches its share of the total.
        if runs.len() + 1 < parts && bytes * parts >= total * (runs.len() + 1) {
            runs.push((start, &documents[start..=row]));
            start = row + 1;
        }
    }
    if start < documents.len() || runs.is_empty() {
        runs.push((start, &documents[start..]));
    }
    runs
}

/// The query the sections describe.
fn query_builder(sections: &[SectionSpec]) -> LazyQueryBuilder<String> {
    let section = |spec: &SectionSpec| {
        let selector = spec.selector.clone();
        if spec.first {
            LazyQuery::first(selector, spec.save())
        } else {
            LazyQuery::all(selector, spec.save())
        }
    };
    // `Layout::new` checked the order: appending each section under its
    // parent rebuilds the same section indexes.
    let mut builder = section(&sections[0]);
    for spec in &sections[1..] {
        let parent = spec.parent.expect("only the root has no parent");
        builder.append(QuerySectionId(parent), section(spec));
    }
    builder
}

type Strings = MutableBinaryViewArray<str>;

/// The values of one section: one per document for the root, one per match
/// of the parent section for a child.
struct Column {
    first: bool,
    /// For `Query.all`, where each value's matches start in `rows`.
    offsets: Offsets<i64>,
    validity: MutableBitmap,
    rows: Rows,
}

impl Column {
    fn new(layout: &Layout<'_>, attributes: Option<&[String]>) -> Self {
        Self {
            first: layout.spec.first,
            offsets: Offsets::new(),
            validity: MutableBitmap::new(),
            rows: Rows::new(layout, attributes),
        }
    }

    fn push_null(&mut self) {
        if self.first {
            self.rows.push_null();
        } else {
            self.offsets.extend_constant(1);
        }
        self.validity.push(false);
    }

    /// Push the value of one parent: its matches, if any.
    fn push(&mut self, matches: Option<Elements<'_, '_, '_>>) -> PolarsResult<()> {
        if self.first {
            match matches.and_then(|mut matches| matches.next()) {
                Some(element) => {
                    self.rows.push(element)?;
                    self.validity.push(true);
                }
                None => self.push_null(),
            }
            return Ok(());
        }
        let start = self.rows.len;
        for element in matches.into_iter().flatten() {
            self.rows.push(element)?;
        }
        self.offsets.try_push(self.rows.len - start)?;
        self.validity.push(true);
        Ok(())
    }

    fn finish(self) -> Box<dyn Array> {
        let validity = self.validity.into();
        let rows = self.rows.finish();
        if self.first {
            return rows.with_validity(validity).boxed();
        }
        let dtype = ArrowDataType::LargeList(Box::new(ArrowField::new(
            LIST_VALUES_NAME,
            rows.dtype().clone(),
            true,
        )));
        ListArray::<i64>::new(dtype, self.offsets.into(), rows.boxed(), validity).boxed()
    }
}

/// One struct per match.
struct Rows {
    len: usize,
    tag: Strings,
    inner_html: Option<Strings>,
    raw_text: Option<Strings>,
    text: Option<Strings>,
    attributes: Attributes,
    /// Child sections, with the field name each is saved under.
    children: Vec<(String, Column)>,
}

impl Rows {
    fn new(layout: &Layout<'_>, attribute_names: Option<&[String]>) -> Self {
        let spec = layout.spec;
        let saved = |saved: bool| saved.then(Strings::new);
        let attributes = match (spec.attributes, attribute_names) {
            (false, _) => Attributes::None,
            (true, Some(names)) => Attributes::Named(
                names
                    .iter()
                    .map(|name| (name.clone(), Strings::new()))
                    .collect(),
            ),
            (true, None) => Attributes::Pairs(Box::new(Pairs::new())),
        };
        Self {
            len: 0,
            tag: Strings::new(),
            inner_html: saved(spec.inner_html),
            raw_text: saved(spec.raw_text),
            text: saved(spec.text),
            attributes,
            children: layout
                .children
                .iter()
                .map(|child| {
                    (
                        child.spec.selector.clone(),
                        Column::new(child, attribute_names),
                    )
                })
                .collect(),
        }
    }

    fn push(&mut self, element: ElementRef<'_, '_, '_>) -> PolarsResult<()> {
        self.len += 1;
        self.tag.push_value(element.name());
        for (column, value) in [
            (&mut self.inner_html, element.inner_html()),
            (&mut self.raw_text, element.raw_text()),
            (&mut self.text, element.text()),
        ] {
            if let Some(column) = column {
                column.push(value);
            }
        }
        self.attributes.push(&element)?;
        for (index, (_, child)) in self.children.iter_mut().enumerate() {
            child.push(element.nested(index))?;
        }
        Ok(())
    }

    fn push_null(&mut self) {
        self.len += 1;
        self.tag.push_null();
        for column in [&mut self.inner_html, &mut self.raw_text, &mut self.text]
            .into_iter()
            .flatten()
        {
            column.push_null();
        }
        self.attributes.push_null();
        for (_, child) in &mut self.children {
            child.push_null();
        }
    }

    fn finish(self) -> StructArray {
        let mut fields: Vec<(String, Box<dyn Array>)> =
            vec![("tag".into(), self.tag.freeze().boxed())];
        for (name, column) in [
            ("inner_html", self.inner_html),
            ("raw_text", self.raw_text),
            ("text", self.text),
        ] {
            if let Some(column) = column {
                fields.push((name.into(), column.freeze().boxed()));
            }
        }
        match self.attributes {
            Attributes::None => {}
            Attributes::Named(columns) => fields.extend(
                columns
                    .into_iter()
                    .map(|(name, column)| (name, column.freeze().boxed())),
            ),
            Attributes::Pairs(pairs) => fields.push(("attributes".into(), pairs.finish())),
        }
        fields.extend(
            self.children
                .into_iter()
                .map(|(name, child)| (name, child.finish())),
        );
        struct_array(self.len, fields)
    }
}

enum Attributes {
    /// The section doesn't save attributes.
    None,
    /// One column per requested attribute.
    Named(Vec<(String, Strings)>),
    /// Every attribute, as name/value pairs.
    Pairs(Box<Pairs>),
}

impl Attributes {
    fn push(&mut self, element: &ElementRef<'_, '_, '_>) -> PolarsResult<()> {
        match self {
            Self::None => {}
            Self::Named(columns) => {
                for (name, column) in columns {
                    column.push(attribute(element, name));
                }
            }
            Self::Pairs(pairs) => pairs.push(element)?,
        }
        Ok(())
    }

    fn push_null(&mut self) {
        match self {
            Self::None => {}
            Self::Named(columns) => columns
                .iter_mut()
                .for_each(|(_, column)| column.push_null()),
            Self::Pairs(pairs) => pairs.push_null(),
        }
    }
}

/// `element`'s attribute `name`, reading `class` and `id` like the Arrow
/// export does.
fn attribute<'h, 'q: 'h>(element: &ElementRef<'_, 'h, 'q>, name: &str) -> Option<&'h str> {
    if name.eq_ignore_ascii_case("class") {
        element.class()
    } else if name.eq_ignore_ascii_case("id") {
        element.id()
    } else {
        element.attribute(name)
    }
}

/// A list of name/value structs per match.
struct Pairs {
    offsets: Offsets<i64>,
    validity: MutableBitmap,
    names: Strings,
    values: Strings,
}

impl Pairs {
    fn new() -> Self {
        Self {
            offsets: Offsets::new(),
            validity: MutableBitmap::new(),
            names: Strings::new(),
            values: Strings::new(),
        }
    }

    fn push(&mut self, element: &ElementRef<'_, '_, '_>) -> PolarsResult<()> {
        let start = self.names.len();
        for (name, value) in [("class", element.class()), ("id", element.id())] {
            if let Some(value) = value {
                self.names.push_value(name);
                self.values.push_value(value);
            }
        }
        for pair in element.attributes().into_iter().flatten() {
            self.names.push_value(pair.key);
            self.values.push(pair.value);
        }
        self.offsets.try_push(self.names.len() - start)?;
        self.validity.push(true);
        Ok(())
    }

    fn push_null(&mut self) {
        self.offsets.extend_constant(1);
        self.validity.push(false);
    }

    fn finish(self) -> Box<dyn Array> {
        let len = self.names.len();
        let pairs = struct_array(
            len,
            vec![
                ("name".into(), self.names.freeze().boxed()),
                ("value".into(), self.values.freeze().boxed()),
            ],
        );
        let dtype = ArrowDataType::LargeList(Box::new(ArrowField::new(
            LIST_VALUES_NAME,
            pairs.dtype().clone(),
            true,
        )));
        ListArray::<i64>::new(
            dtype,
            self.offsets.into(),
            pairs.boxed(),
            self.validity.into(),
        )
        .boxed()
    }
}

/// A struct array of `len` rows with these fields, all nullable.
fn struct_array(len: usize, fields: Vec<(String, Box<dyn Array>)>) -> StructArray {
    let dtype = ArrowDataType::Struct(
        fields
            .iter()
            .map(|(name, values)| ArrowField::new(name.into(), values.dtype().clone(), true))
            .collect(),
    );
    let values = fields.into_iter().map(|(_, values)| values).collect();
    StructArray::new(dtype, len, values, None)
}

// ── The plugin ABI ─────────────────────────────────────────────────────────
//
// The functions polars looks up in the library for a plugin function named
// `parse`. pyo3-polars' `#[polars_expr]` would generate them, but its
// `derive` feature pulls in polars-plan and with it polars-io,
// polars-parquet and polars-ops; written out, they need only polars-ffi.

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

/// Keep `message` for `_polars_plugin_get_last_error_message`, which polars
/// calls when a plugin function leaves its return value empty.
fn set_last_error(message: impl std::fmt::Display) {
    let message = CString::new(message.to_string().replace('\0', "")).unwrap_or_default();
    LAST_ERROR.with(|last| *last.borrow_mut() = message);
}

/// Run `f`, turning an error or a panic into the last error message.
fn report<T>(f: impl FnOnce() -> PolarsResult<T>) -> Option<T> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(value)) => Some(value),
        Ok(Err(err)) => {
            set_last_error(err);
            None
        }
        Err(_) => {
            set_last_error("scah.polars.parse panicked");
            None
        }
    }
}

/// # Safety
/// `ptr` points to `len` readable bytes.
unsafe fn read_kwargs(ptr: *const u8, len: usize) -> PolarsResult<ParseKwargs> {
    // SAFETY: the caller's contract.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    serde_pickle::from_slice(bytes, Default::default())
        .map_err(|err| polars_err!(InvalidOperation: "invalid scah.polars.parse arguments: {err}"))
}

/// The plugin ABI version this library implements.
#[unsafe(no_mangle)]
pub extern "C" fn _polars_plugin_get_version() -> u32 {
    let (major, minor) = polars_ffi::get_version();
    (u32::from(major) << 16) + u32::from(minor)
}

/// The message of the last error on this thread.
#[unsafe(no_mangle)]
pub extern "C" fn _polars_plugin_get_last_error_message() -> *const c_char {
    LAST_ERROR.with(|last| last.borrow().as_ptr())
}

/// The output field of `parse`.
///
/// # Safety
/// Polars passes `len` valid schemas at `fields`, a writable empty schema at
/// `return_value`, and `kwargs_len` bytes at `kwargs_ptr`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _polars_plugin_field_parse(
    fields: *mut ArrowSchema,
    len: usize,
    return_value: *mut ArrowSchema,
    kwargs_ptr: *const u8,
    kwargs_len: usize,
) {
    let field = report(|| {
        // SAFETY: the caller's contract.
        let inputs = unsafe { std::slice::from_raw_parts(fields, len) }
            .iter()
            // SAFETY: each schema is valid.
            .map(|field| unsafe { ffi::import_field_from_c(field) }.map(|f| Field::from(&f)))
            .collect::<PolarsResult<Vec<_>>>()?;
        // SAFETY: the caller's contract.
        let kwargs = unsafe { read_kwargs(kwargs_ptr, kwargs_len) }?;
        parse_output(&inputs, kwargs)
    });
    if let Some(field) = field {
        let schema = ffi::export_field_to_c(&field.to_arrow(CompatLevel::newest()));
        // SAFETY: `return_value` is writable and holds an empty schema.
        unsafe { *return_value = schema };
    }
}

/// Run `parse` on a chunk of the column.
///
/// # Safety
/// Polars passes `input_len` valid series at `inputs`, `kwargs_len` bytes at
/// `kwargs_ptr`, and a writable empty series at `return_value`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _polars_plugin_parse(
    inputs: *mut SeriesExport,
    input_len: usize,
    kwargs_ptr: *const u8,
    kwargs_len: usize,
    return_value: *mut SeriesExport,
    _context: *mut CallerContext,
) {
    let series = report(|| {
        // SAFETY: the caller's contract; this takes ownership of the inputs.
        let inputs = unsafe { import_series_buffer(inputs, input_len) }?;
        // SAFETY: the caller's contract.
        let kwargs = unsafe { read_kwargs(kwargs_ptr, kwargs_len) }?;
        parse(&inputs, kwargs)
    });
    if let Some(series) = series {
        // SAFETY: `return_value` is writable and holds an empty series.
        unsafe { *return_value = export_series(&series) };
    }
}

#[cfg(test)]
mod tests {
    use super::split_by_bytes;

    fn split(sizes: &[Option<usize>], parts: usize) -> Vec<(usize, usize)> {
        let text = "x".repeat(sizes.iter().flatten().copied().max().unwrap_or(0));
        let documents: Vec<_> = sizes.iter().map(|size| size.map(|n| &text[..n])).collect();
        let total = sizes.iter().flatten().sum();
        split_by_bytes(&documents, parts, total)
            .into_iter()
            .map(|(start, run)| (start, run.len()))
            .collect()
    }

    #[test]
    fn one_part_is_everything() {
        assert_eq!(split(&[Some(5), None, Some(1)], 1), [(0, 3)]);
        assert_eq!(split(&[], 1), [(0, 0)]);
    }

    #[test]
    fn parts_balance_bytes_and_cover_every_row() {
        assert_eq!(split(&[Some(1); 8], 4), [(0, 2), (2, 2), (4, 2), (6, 2)]);
        assert_eq!(
            split(&[Some(10), Some(1), Some(1), Some(10), None], 2),
            [(0, 2), (2, 3)]
        );
    }

    #[test]
    fn never_more_parts_than_asked_or_empty_parts() {
        let runs = split(&[Some(100), Some(1), Some(1)], 3);
        assert!(runs.len() <= 3);
        assert!(runs.iter().all(|&(_, len)| len > 0));
        assert_eq!(runs.iter().map(|&(_, len)| len).sum::<usize>(), 3);
    }
}
