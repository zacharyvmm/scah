use super::column::View;
use super::*;
use crate::{OwnedQuery, OwnedStore};
use arrow_array::ffi::{FFI_ArrowArray, FFI_ArrowSchema, from_ffi};
use arrow_array::ffi_stream::RecordBatchStreamReader;
use arrow_array::{
    Array, RecordBatch, RecordBatchReader, StringViewArray, StructArray, UInt32Array,
};
use arrow_schema::DataType;
use scah::lazy::{LazyQuery, LazyQueryBuilder};
use scah::{Buffer, Save};
use std::ptr;

fn parse(html: &str, queries: Vec<LazyQueryBuilder<String>>) -> Arc<OwnedStore> {
    let queries: Vec<_> = queries
        .into_iter()
        .map(|query| OwnedQuery::build(query).unwrap())
        .collect();
    let queries: Vec<_> = queries.iter().collect();
    Arc::new(OwnedStore::parse(html.to_owned(), &queries).unwrap())
}

fn table(
    store: &Arc<OwnedStore>,
    selector: &str,
    parent: Option<&str>,
    attributes: &[&str],
) -> Arc<ArrowTable> {
    Arc::new(ArrowTable::build(store.clone(), selector, parent, attributes).unwrap())
}

/// Move an exported schema into arrow-rs, leaving `schema` released.
fn import_schema(schema: &mut ArrowSchema) -> FFI_ArrowSchema {
    // SAFETY: `ArrowSchema` has the C layout that `FFI_ArrowSchema`
    // mirrors, and `from_raw` moves the struct out.
    unsafe { FFI_ArrowSchema::from_raw(ptr::from_mut(schema).cast()) }
}

/// Move an exported array into arrow-rs, leaving `array` released.
fn import_array(array: &mut ArrowArray) -> FFI_ArrowArray {
    // SAFETY: as in `import_schema`.
    unsafe { FFI_ArrowArray::from_raw(ptr::from_mut(array).cast()) }
}

/// Import an exported table with arrow-rs, validating every buffer.
fn import(table: &Arc<ArrowTable>) -> StructArray {
    let (mut schema, mut array) = table.export();
    let schema = import_schema(&mut schema);
    let array = import_array(&mut array);
    // SAFETY: both structs were just exported by `table`.
    let data = unsafe { from_ffi(array, &schema) }.unwrap();
    data.validate_full().unwrap();
    StructArray::from(data)
}

/// One row: the element id, the parent id, then tag, inner HTML, raw
/// text, text, and the attributes.
type Row = (u32, Option<u32>, Vec<Option<String>>);

/// The rows the table should hold, read through the handle API.
fn expected(
    store: &OwnedStore,
    selector: &str,
    parent: Option<&str>,
    attributes: &[&str],
) -> Vec<Row> {
    let doc = store.store();
    let id = |element: scah::ElementRef| element.index().index() as u32;
    let mut elements = Vec::new();
    match parent {
        None => elements.extend(doc.get(selector).into_iter().flatten().map(|e| (None, e))),
        Some(parent) => {
            for parent in doc.get(parent).into_iter().flatten() {
                let children = parent.get(selector).into_iter().flatten();
                elements.extend(children.map(|e| (Some(id(parent)), e)));
            }
        }
    }
    elements
        .into_iter()
        .map(|(parent, element)| {
            let mut values = vec![
                Some(element.name()),
                element.inner_html(),
                element.raw_text(),
                element.text(),
            ];
            values.extend(attributes.iter().map(|&name| match name {
                _ if name.eq_ignore_ascii_case("id") => element.id(),
                _ if name.eq_ignore_ascii_case("class") => element.class(),
                name => element.attribute(name),
            }));
            let values = values.into_iter().map(|v| v.map(str::to_owned)).collect();
            (id(element), parent, values)
        })
        .collect()
}

fn strings(column: &dyn Array) -> &StringViewArray {
    column.as_any().downcast_ref().unwrap()
}

/// The rows of an imported table.
fn rows(batch: &StructArray, has_parent: bool) -> Vec<Row> {
    let ids: &UInt32Array = batch.column(0).as_any().downcast_ref().unwrap();
    let parents: Option<&UInt32Array> =
        has_parent.then(|| batch.column(1).as_any().downcast_ref().unwrap());
    let first = 1 + usize::from(has_parent);
    (0..batch.len())
        .map(|row| {
            let values = batch.columns()[first..]
                .iter()
                .map(|column| {
                    let column = strings(column);
                    column.is_valid(row).then(|| column.value(row).to_owned())
                })
                .collect();
            (ids.value(row), parents.map(|p| p.value(row)), values)
        })
        .collect()
}

/// Check the schema, every cell, and that the views borrow the store's
/// buffers. Returns the imported table.
fn check(
    store: &Arc<OwnedStore>,
    selector: &str,
    parent: Option<&str>,
    attributes: &[&str],
) -> StructArray {
    let table = table(store, selector, parent, attributes);
    let batch = import(&table);

    let mut names = vec!["index"];
    names.extend(parent.map(|_| "parent"));
    names.extend(["tag", "inner_html", "raw_text", "text"]);
    names.extend(attributes);
    let fields = batch.fields();
    assert_eq!(fields.iter().map(|f| f.name()).collect::<Vec<_>>(), names);
    for field in fields.iter() {
        let nullable = !matches!(field.name().as_str(), "index" | "parent" | "tag");
        let data_type = match field.name().as_str() {
            "index" | "parent" => DataType::UInt32,
            _ => DataType::Utf8View,
        };
        assert_eq!(field.is_nullable(), nullable, "{field:?}");
        assert_eq!(field.data_type(), &data_type, "{field:?}");
    }

    let expected = expected(store, selector, parent, attributes);
    assert_eq!(table.num_rows(), expected.len());
    assert_eq!(rows(&batch, parent.is_some()), expected);

    let doc = store.store();
    let shared = [Buffer::Html, Buffer::RawText, Buffer::Text].map(|buffer| doc.buffer(buffer));
    for column in &batch.columns()[1 + usize::from(parent.is_some())..] {
        // At most one data buffer, one of the store's own, never a copy.
        let buffers = strings(column).data_buffers();
        assert!(buffers.len() <= 1);
        for buffer in buffers.iter() {
            assert!(
                shared
                    .iter()
                    .any(|original| buffer.as_ptr() == original.as_ptr()
                        && buffer.len() == original.len()),
                "copied a buffer"
            );
        }
    }
    batch
}

const LINKS: &str = concat!(
    "<ul id='list' class='links'>",
    "<li><a id='first' class='link external' title='' ",
    "href='https://example.com/a/very/long/path'>Fish &amp; chips are great</a></li>",
    "<li><a href='/b' disabled name='twelve-bytes' title='thirteen-byte'>B</a></li>",
    "<li><a href='/é' class=''>héllo wörld ✓ 日本語のテキスト &lt;tag&gt;</a></li>",
    "<li><a>  <b>nested</b>   text  spread   over   many  spaces  </a></li>",
    "<li><a href='/x'></a></li>",
    "</ul>",
);

const ATTRIBUTES: &[&str] = &["id", "class", "href", "name", "title", "disabled"];

#[test]
fn exports_every_value_of_the_handle_api() {
    let store = parse(
        LINKS,
        vec![
            LazyQuery::all("a".to_owned(), Save::all()),
            LazyQuery::all("li".to_owned(), Save::only_text()),
        ],
    );
    let batch = check(&store, "a", None, ATTRIBUTES);
    assert_eq!(batch.len(), 5);

    let column = |name| strings(batch.column_by_name(name).unwrap());
    // Missing, empty, and valueless values.
    assert!(column("id").is_null(1));
    assert_eq!(column("class").value(2), "");
    assert!(column("class").is_valid(2));
    assert_eq!(column("title").value(0), "");
    assert!(column("disabled").is_null(1));
    assert!(column("href").is_null(3));
    // Inline (12 bytes) and out-of-line (13 bytes) views.
    assert_eq!(column("name").value(1), "twelve-bytes");
    assert_eq!(column("title").value(1), "thirteen-byte");
    // Entities are decoded only in `text`.
    assert_eq!(column("text").value(0), "Fish & chips are great");
    assert_eq!(column("raw_text").value(0), "Fish &amp; chips are great");
    assert_eq!(
        column("text").value(2),
        "héllo wörld ✓ 日本語のテキスト <tag>"
    );
    assert_eq!(column("text").value(4), "");
    assert!(column("text").is_valid(4));
    assert_eq!(column("tag").null_count(), 0);
    assert_eq!(column("text").null_count(), 0);

    // Unsaved content and attributes are null.
    let batch = check(&store, "li", None, &["id", "class", "href"]);
    assert_eq!(batch.len(), 5);
    for name in ["inner_html", "raw_text", "id", "class", "href"] {
        assert_eq!(batch.column_by_name(name).unwrap().null_count(), 5);
    }
    assert_eq!(batch.column_by_name("text").unwrap().null_count(), 0);
}

#[test]
fn attribute_names_are_matched_case_insensitively() {
    let store = parse(LINKS, vec![LazyQuery::all("a".to_owned(), Save::all())]);
    let batch = import(&table(&store, "a", None, &["ID", "Class", "HREF"]));
    let first = 5;
    assert_eq!(strings(batch.column(first)).value(0), "first");
    assert_eq!(strings(batch.column(first + 1)).value(0), "link external");
    assert_eq!(strings(batch.column(first + 2)).value(1), "/b");
}

#[test]
fn exports_child_sections_with_their_parents() {
    let html = concat!(
        "<section id='products'>",
        "<div class='product'><h1>Product #1 with a long name</h1>",
        "<span class='rating'>4/5</span></div>",
        "<div class='product'><h1>Short</h1></div>",
        "<div class='product'></div>",
        "<div class='product'><h1>Fourth &amp; last product</h1>",
        "<span class='rating'>5/5</span></div>",
        "</section>",
    );
    let store = parse(
        html,
        vec![
            LazyQuery::all("div.product".to_owned(), Save::none()).then(|product| {
                [
                    product.first("h1".to_owned(), Save::only_text()),
                    product.all("span.rating".to_owned(), Save::all()),
                ]
            }),
        ],
    );

    let products = check(&store, "div.product", None, &[]);
    assert_eq!(products.len(), 4);
    let product_ids: &UInt32Array = products.column(0).as_any().downcast_ref().unwrap();

    let titles = check(&store, "h1", Some("div.product"), &["class"]);
    assert_eq!(titles.len(), 3);
    let parents: &UInt32Array = titles.column(1).as_any().downcast_ref().unwrap();
    assert_eq!(
        parents.values().to_vec(),
        [
            product_ids.value(0),
            product_ids.value(1),
            product_ids.value(3)
        ]
    );
    assert_eq!(
        strings(titles.column_by_name("text").unwrap()).value(2),
        "Fourth & last product"
    );

    let ratings = check(&store, "span.rating", Some("div.product"), &["class"]);
    assert_eq!(ratings.len(), 2);

    // Child sections are not top-level, and vice versa.
    assert_eq!(check(&store, "h1", None, &[]).len(), 0);
    assert_eq!(check(&store, "div.product", Some("h1"), &[]).len(), 0);
}

#[test]
fn unmatched_selectors_yield_empty_tables() {
    let store = parse(
        "<p>text</p>",
        vec![LazyQuery::all("a".to_owned(), Save::all())],
    );
    for (selector, parent) in [("a", None), ("never", None), ("a", Some("never"))] {
        let batch = check(&store, selector, parent, &["href"]);
        assert_eq!(batch.len(), 0);
        assert_eq!(batch.num_columns(), 6 + usize::from(parent.is_some()));
    }
}

#[test]
fn rejects_conflicting_column_names() {
    let store = parse("<a></a>", vec![LazyQuery::all("a".to_owned(), Save::all())]);
    let build = |parent, attributes: &[&str]| {
        ArrowTable::build(store.clone(), "a", parent, attributes).err()
    };
    for name in ["index", "tag", "inner_html", "raw_text", "text"] {
        assert_eq!(
            build(None, &[name]),
            Some(ArrowError::DuplicateColumn(name.to_owned()))
        );
    }
    assert_eq!(
        build(Some("p"), &["parent"]),
        Some(ArrowError::DuplicateColumn("parent".to_owned()))
    );
    assert_eq!(build(None, &["parent"]), None);
    assert_eq!(
        build(None, &["href", "href"]),
        Some(ArrowError::DuplicateColumn("href".to_owned()))
    );
    assert_eq!(build(None, &["href", "HREF"]), None);
    assert_eq!(
        build(None, &["a\0b"]),
        Some(ArrowError::InvalidColumnName("a\0b".to_owned()))
    );
}

#[test]
fn exports_outlive_the_store_and_the_table() {
    let store = parse(LINKS, vec![LazyQuery::all("a".to_owned(), Save::all())]);
    let expected = expected(&store, "a", None, ATTRIBUTES);
    let table = table(&store, "a", None, ATTRIBUTES);
    drop(store);

    let first = import(&table);
    let second = import(&table);
    drop(table);
    assert_eq!(rows(&first, false), expected);
    assert_eq!(rows(&second, false), expected);
    // Both exports share the same buffers rather than copies.
    assert_eq!(
        strings(first.column(2)).data_buffers()[0].as_ptr(),
        strings(second.column(2)).data_buffers()[0].as_ptr()
    );
}

#[test]
fn consumers_may_move_children_out() {
    let store = parse(LINKS, vec![LazyQuery::all("a".to_owned(), Save::all())]);
    let expected: Vec<_> = expected(&store, "a", None, &[])
        .into_iter()
        .map(|(_, _, values)| values[3].clone())
        .collect();
    let table = table(&store, "a", None, &[]);
    let (mut schema, mut array) = table.export();
    drop((store, table));

    // Move the `text` column's schema and array out of their parents, as
    // the spec allows, then release the parents.
    // SAFETY: both parents are live exports with five children.
    let (mut text_schema, mut text_array) = unsafe {
        let schema_slot = *schema.children.add(4);
        let array_slot = *array.children.add(4);
        let moved = (ptr::read(schema_slot), ptr::read(array_slot));
        (*schema_slot).release = None;
        (*array_slot).release = None;
        (schema.release.unwrap())(&mut schema);
        (array.release.unwrap())(&mut array);
        moved
    };
    assert!(schema.release.is_none() && array.release.is_none());

    let text_schema = import_schema(&mut text_schema);
    // SAFETY: the moved child is still a live export.
    let data = unsafe { from_ffi(import_array(&mut text_array), &text_schema) }.unwrap();
    data.validate_full().unwrap();
    let text = StringViewArray::from(data);
    let actual: Vec<_> = text.iter().map(|v| v.map(str::to_owned)).collect();
    assert_eq!(actual, expected);
}

#[test]
fn streams_one_batch() {
    let store = parse(LINKS, vec![LazyQuery::all("a".to_owned(), Save::all())]);
    let expected = expected(&store, "a", None, ATTRIBUTES);
    let table = table(&store, "a", None, ATTRIBUTES);
    let mut stream = table.export_stream();
    drop((store, table));

    // SAFETY: `ArrowArrayStream` has the C layout `FFI_ArrowArrayStream`
    // mirrors, and `from_raw` moves the stream out.
    let reader =
        unsafe { RecordBatchStreamReader::from_raw(ptr::from_mut(&mut stream).cast()) }.unwrap();
    assert!(stream.release.is_none());
    let schema = reader.schema();
    assert_eq!(schema.fields().len(), 4 + 1 + ATTRIBUTES.len());
    let batches: Vec<RecordBatch> = reader.collect::<Result<_, _>>().unwrap();
    assert_eq!(batches.len(), 1);
    let batch = StructArray::from(batches[0].clone());
    batch.to_data().validate_full().unwrap();
    assert_eq!(rows(&batch, false), expected);
}

#[test]
fn stream_can_be_released_unread() {
    let store = parse(LINKS, vec![LazyQuery::all("a".to_owned(), Save::all())]);
    let mut stream = table(&store, "a", None, &[]).export_stream();
    // SAFETY: a live exported stream; released once.
    unsafe {
        let mut schema = ArrowSchema::released();
        assert_eq!((stream.get_schema.unwrap())(&mut stream, &mut schema), 0);
        assert!((stream.get_last_error.unwrap())(&mut stream).is_null());
        (schema.release.unwrap())(&mut schema);
        (stream.release.unwrap())(&mut stream);
    }
    assert!(stream.release.is_none());
}

#[test]
fn columns_list_only_the_buffers_they_use() {
    let store = parse(
        "<a href='/a/long/enough/path'>some text long enough to leave the view</a>",
        vec![LazyQuery::all("a".to_owned(), Save::only_text())],
    );
    let table = Arc::new(ArrowTable::build(store, "a", None, &["href", "title"]).unwrap());
    let batch = import(&table);
    let buffers = |name: &str| {
        strings(batch.column_by_name(name).unwrap())
            .data_buffers()
            .len()
    };
    // Short values are inlined, so `tag` needs no data buffer at all.
    assert_eq!(buffers("tag"), 0);
    assert_eq!(buffers("href"), 1);
    assert_eq!(buffers("text"), 1);
    assert_eq!(buffers("raw_text"), 0);
    assert_eq!(buffers("title"), 0);
    assert_eq!(
        strings(batch.column_by_name("text").unwrap()).value(0),
        "some text long enough to leave the view"
    );
}

#[test]
fn views_follow_the_layout() {
    let len = |view: &View| i32::from_ne_bytes(view.0[..4].try_into().unwrap());
    // Inlined from a fixed window of the buffer, and from its very end.
    let buffer = b"<twelve-bytes>, then more than twelve bytes";
    let inline = View::new(buffer, 1..13);
    assert_eq!(len(&inline), 12);
    assert_eq!(&inline.0[4..], b"twelve-bytes");
    for buffer in [&b"<abc> and more than twelve bytes"[..], b"<abc"] {
        let short = View::new(buffer, 1..4);
        assert_eq!(len(&short), 3);
        assert_eq!(&short.0[4..], b"abc\0\0\0\0\0\0\0\0\0", "zero-padded");
    }
    assert_eq!(View::new(buffer, 5..5).0, [0; 16], "empty");

    let reference = View::new(b"0123456thirteen-byte", 7..20);
    let field =
        |range: std::ops::Range<usize>| i32::from_ne_bytes(reference.0[range].try_into().unwrap());
    assert_eq!(field(0..4), 13);
    assert_eq!(&reference.0[4..8], b"thir");
    assert_eq!(field(8..12), 0, "the column's only data buffer");
    assert_eq!(field(12..16), 7);
}

#[test]
fn tables_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ArrowTable>();
}
