"""Arrow export: Store.to_arrow and the Arrow PyCapsule Interface.

Every value is checked against the element handle API.
"""

import gc

import pytest

from scah import Query, Save, parse

pa = pytest.importorskip("pyarrow")

LINKS = (
    "<ul id='list' class='links'>"
    "<li><a id='first' class='link external' title='' "
    "href='https://example.com/a/very/long/path'>Fish &amp; chips are great</a></li>"
    "<li><a href='/b' disabled name='twelve-bytes' title='thirteen-byte'>B</a></li>"
    "<li><a href='/é' class=''>héllo wörld ✓ 日本語のテキスト &lt;tag&gt;</a></li>"
    "<li><a>  <b>nested</b>   text  spread   over   many  spaces  </a></li>"
    "<li><a href='/x'></a></li>"
    "</ul>"
)

ATTRIBUTES = ["id", "class", "href", "name", "title", "disabled"]

CATALOG = (
    "<section id='products'>"
    "<div class='product'><h1>Product #1 with a long name</h1>"
    "<span class='rating'>4/5</span></div>"
    "<div class='product'><h1>Short</h1></div>"
    "<div class='product'></div>"
    "<div class='product'><h1>Fourth &amp; last product</h1>"
    "<span class='rating'>5/5</span></div>"
    "</section>"
)


def links_store():
    return parse(
        LINKS,
        [
            Query.all("a", Save.all()).build(),
            Query.all("li", Save.only_text()).build(),
        ],
    )


def catalog_store():
    query = (
        Query.all("div.product", Save.none())
        .then(
            lambda product: [
                product.first("h1", Save.only_text()),
                product.all("span.rating", Save.all()),
            ]
        )
        .build()
    )
    return parse(CATALOG, [query])


def handle_row(element, attributes):
    """One row of string columns, read through the handle API."""
    row = {
        "tag": element.name,
        "inner_html": element.inner_html,
        "raw_text": element.raw_text,
        "text": element.text,
    }
    for name in attributes:
        if name == "id":
            row[name] = element.id
        elif name == "class":
            row[name] = element.class_name
        else:
            row[name] = element.get_attribute(name)
    return row


def children(element, selector):
    """Child matches through the handle API, which raises when there are none."""
    try:
        return element.get(selector)
    except ValueError:
        return []


def handle_rows(elements, attributes):
    return [handle_row(element, attributes) for element in elements]


def string_columns(rows):
    """Drop the uint32 id columns from pylist rows."""
    return [
        {k: v for k, v in row.items() if k not in ("index", "parent")} for row in rows
    ]


# ── Table and schema ───────────────────────────────────────────────────────


def test_schema_and_values_match_the_handle_api():
    store = links_store()
    table = store.to_arrow("a", attributes=ATTRIBUTES)
    assert len(table) == table.num_rows == 5
    names = ["index", "tag", "inner_html", "raw_text", "text", *ATTRIBUTES]
    assert table.column_names == names

    batch = pa.record_batch(table)
    assert batch.schema.names == names
    assert batch.schema.field("index").type == pa.uint32()
    assert not batch.schema.field("index").nullable
    assert not batch.schema.field("tag").nullable
    for name in names[1:]:
        field = batch.schema.field(name)
        assert field.type == pa.string_view()
        assert field.nullable == (name != "tag")
    batch.validate(full=True)

    expected = handle_rows(store.get("a"), ATTRIBUTES)
    assert string_columns(batch.to_pylist()) == expected
    assert pa.table(table).to_pylist() == batch.to_pylist()

    # Spot checks: missing, empty, valueless, entities, multi-byte UTF-8.
    rows = batch.to_pylist()
    assert rows[1]["id"] is None
    assert rows[2]["class"] == ""
    assert rows[1]["disabled"] is None
    assert rows[1]["name"] == "twelve-bytes"
    assert rows[1]["title"] == "thirteen-byte"
    assert rows[0]["text"] == "Fish & chips are great"
    assert rows[0]["raw_text"] == "Fish &amp; chips are great"
    assert rows[2]["text"] == "héllo wörld ✓ 日本語のテキスト <tag>"
    assert rows[4]["text"] == ""


def test_unsaved_values_are_null():
    store = links_store()
    batch = pa.record_batch(store.to_arrow("li", attributes=["href"]))
    assert batch.num_rows == 5
    for name in ("inner_html", "raw_text", "href"):
        assert batch.column(name).null_count == 5
    assert batch.column("text").null_count == 0
    assert string_columns(batch.to_pylist()) == handle_rows(store.get("li"), ["href"])


def test_ids_are_distinct_and_ordered():
    batch = pa.record_batch(links_store().to_arrow("a"))
    ids = batch.column("index").to_pylist()
    assert ids == sorted(set(ids))


def test_nested_tables_join_on_parent():
    store = catalog_store()
    products = pa.record_batch(store.to_arrow("div.product"))
    titles = pa.record_batch(store.to_arrow("h1", parent="div.product"))
    ratings = pa.record_batch(
        store.to_arrow("span.rating", parent="div.product", attributes=["class"])
    )
    assert titles.schema.names[:2] == ["index", "parent"]
    assert titles.schema.field("parent").type == pa.uint32()

    product_ids = products.column("index").to_pylist()
    expected_titles, expected_ratings = [], []
    expected_title_parents, expected_rating_parents = [], []
    for product_id, product in zip(product_ids, store.get("div.product")):
        for h1 in children(product, "h1"):
            expected_titles.append(handle_row(h1, []))
            expected_title_parents.append(product_id)
        for rating in children(product, "span.rating"):
            expected_ratings.append(handle_row(rating, ["class"]))
            expected_rating_parents.append(product_id)

    assert string_columns(titles.to_pylist()) == expected_titles
    assert titles.column("parent").to_pylist() == expected_title_parents
    assert string_columns(ratings.to_pylist()) == expected_ratings
    assert ratings.column("parent").to_pylist() == expected_rating_parents


def test_empty_results_keep_the_schema():
    store = links_store()
    for kwargs in ({}, {"parent": "never"}):
        table = store.to_arrow("never", attributes=["href"], **kwargs)
        assert len(table) == 0
        batch = pa.record_batch(table)
        assert batch.num_rows == 0
        assert len(batch.schema) == 6 + len(kwargs)
        assert batch.schema.field("href").type == pa.string_view()


def test_conflicting_column_names_are_rejected():
    store = links_store()
    with pytest.raises(ValueError, match="duplicate column name `text`"):
        store.to_arrow("a", attributes=["text"])
    with pytest.raises(ValueError, match="duplicate column name `href`"):
        store.to_arrow("a", attributes=["href", "href"])
    with pytest.raises(ValueError, match="duplicate column name `parent`"):
        store.to_arrow("h1", parent="div", attributes=["parent"])
    with pytest.raises(TypeError):
        store.to_arrow("a", attributes="href")


# ── PyCapsule protocol and lifetimes ───────────────────────────────────────


def test_capsules_can_be_dropped_unconsumed():
    table = links_store().to_arrow("a", attributes=ATTRIBUTES)
    for _ in range(3):
        schema, array = table.__arrow_c_array__()
        del schema, array
    schema = table.__arrow_c_schema__()
    stream = table.__arrow_c_stream__()
    del schema, stream
    gc.collect()
    assert pa.record_batch(table).num_rows == 5


def test_stream_import():
    store = links_store()
    reader = pa.RecordBatchReader.from_stream(store.to_arrow("a", attributes=["href"]))
    batches = list(reader)
    assert len(batches) == 1
    assert string_columns(batches[0].to_pylist()) == handle_rows(store.get("a"), ["href"])


def test_imports_outlive_the_store_and_the_table():
    store = links_store()
    expected = handle_rows(store.get("a"), ATTRIBUTES)
    table = store.to_arrow("a", attributes=ATTRIBUTES)
    batch = pa.record_batch(table)
    arrow_table = pa.table(table)
    del store, table
    gc.collect()
    assert string_columns(batch.to_pylist()) == expected
    assert string_columns(arrow_table.to_pylist()) == expected


def test_pyarrow_imports_without_copying():
    """Every import references the store's own buffers, and each column lists
    only the buffers its values point into."""
    store = links_store()
    table = store.to_arrow("a", attributes=["href"])
    first = pa.record_batch(table)
    second = pa.record_batch(table)
    html = LINKS.encode()

    # Every tag is short enough to live inside its view.
    assert len(first.column("tag").buffers()) == 2
    for name, from_html in (
        ("inner_html", True),
        ("href", True),
        ("raw_text", False),
        ("text", False),
    ):
        a = first.column(name).buffers()
        b = second.column(name).buffers()
        # [validity, views, the one data buffer the column uses]
        assert len(a) == 3
        assert (a[2].to_pybytes() == html) == from_html
        assert a[2].address == b[2].address


# ── polars ─────────────────────────────────────────────────────────────────


def test_polars_import_matches_the_handle_api():
    pl = pytest.importorskip("polars")
    store = links_store()
    expected = handle_rows(store.get("a"), ATTRIBUTES)
    table = store.to_arrow("a", attributes=ATTRIBUTES)

    df = pl.DataFrame(table)
    assert df.columns == table.column_names
    assert df.schema["index"] == pl.UInt32
    assert df.schema["tag"] == pl.String
    assert string_columns(df.to_dicts()) == expected

    del store, table
    gc.collect()
    assert string_columns(df.to_dicts()) == expected


def test_polars_imports_without_copying():
    """polars keeps the exported views and data buffers: exporting its
    columns back to pyarrow yields the addresses pyarrow imported."""
    pl = pytest.importorskip("polars")
    table = links_store().to_arrow("a", attributes=["href"])
    imported = pa.record_batch(table)
    round_trip = pl.DataFrame(table).to_arrow(compat_level=pl.CompatLevel.newest())
    for name in ("tag", "inner_html", "raw_text", "text", "href"):
        [chunk] = round_trip.column(name).chunks
        assert chunk.type == pa.string_view()
        ours = imported.column(name).buffers()[1:]
        theirs = chunk.buffers()[1:]
        assert len(ours) == len(theirs)
        for x, y in zip(ours, theirs):
            if x.size:
                assert (x.address, x.size) == (y.address, y.size)


def test_polars_nested_join():
    pl = pytest.importorskip("polars")
    store = catalog_store()
    products = pl.DataFrame(store.to_arrow("div.product"))
    titles = pl.DataFrame(store.to_arrow("h1", parent="div.product")).select(
        "parent", title="text"
    )
    ratings = pl.DataFrame(store.to_arrow("span.rating", parent="div.product")).select(
        "parent", rating="text"
    )
    joined = (
        products.select("index")
        .join(titles, left_on="index", right_on="parent", how="left")
        .join(ratings, left_on="index", right_on="parent", how="left")
    )
    del store
    gc.collect()

    expected = [
        ("Product #1 with a long name", "4/5"),
        ("Short", None),
        (None, None),
        ("Fourth & last product", "5/5"),
    ]
    assert list(zip(joined["title"], joined["rating"])) == expected
