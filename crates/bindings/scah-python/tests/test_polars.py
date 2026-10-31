"""scah.polars: the polars expression plugin.

Every value is checked against `scah.parse` and the element handle API.
"""

import pytest
import scah
from scah import Query, Save

pl = pytest.importorskip("polars")
from scah.polars import parse  # noqa: E402

PAGES = [
    (
        "<ul id='list'>"
        "<li><a id='first' class='link external' href='https://example.com/a/very/long/path'"
        " title=''>Fish &amp; chips are great</a></li>"
        "<li><a href='/b' disabled data-x='12'>B</a></li>"
        "<li><a href='/é' class=''>héllo wörld ✓ 日本語のテキスト &lt;tag&gt;</a></li>"
        "<li><a>  <b>nested</b>   text  spread   over   many  spaces  </a></li>"
        "</ul>"
    ),
    None,
    "<p>no links here</p>",
    "",
    "<a href='/only'>only</a>",
]

CATALOG = [
    (
        "<section>"
        "<div class='product'><h1>Product #1 with a long name</h1>"
        "<span class='rating'>4/5</span><span class='rating'>5/5</span></div>"
        "<div class='product'><h1>Short</h1></div>"
        "<div class='product'></div>"
        "</section>"
    ),
    None,
    "<div class='product'><span class='rating'>1/5</span></div>",
]


PAIRS = pl.List(pl.Struct({"name": pl.String, "value": pl.String}))


def frame(pages):
    return pl.DataFrame({"html": pages}, schema={"html": pl.String})


def run(pages, query, **kwargs):
    return frame(pages).select(parse(pl.col("html"), query, **kwargs).alias("r"))["r"]


def all_pairs(element):
    """Every attribute as name/value pairs: class, id, then the rest."""
    pairs = []
    if element.class_name is not None:
        pairs.append({"name": "class", "value": element.class_name})
    if element.id is not None:
        pairs.append({"name": "id", "value": element.id})
    for name, value in (element.attributes or {}).items():
        pairs.append({"name": name, "value": value})
    return pairs


def handle_row(element, save, attributes=None):
    row = {"tag": element.name}
    if save.get("inner_html"):
        row["inner_html"] = element.inner_html
    if save.get("raw_text"):
        row["raw_text"] = element.raw_text
    if save.get("text"):
        row["text"] = element.text
    if save.get("attributes", True):
        if attributes is None:
            row["attributes"] = all_pairs(element)
        else:
            for name in attributes:
                if name == "id":
                    row[name] = element.id
                elif name == "class":
                    row[name] = element.class_name
                else:
                    row[name] = element.get_attribute(name)
    return row


def matches(html, query, selector):
    if html is None:
        return None
    return scah.parse(html, [query]).get(selector) or []


# ── Shape and values ───────────────────────────────────────────────────────


@pytest.mark.parametrize(
    ("save", "flags"),
    [
        (Save.all(), {"inner_html": True, "raw_text": True, "text": True}),
        (Save.only_text(), {"text": True}),
        (Save.only_inner_html(), {"inner_html": True}),
        (Save.none(), {}),
        (Save.name_only(), {"attributes": False}),
        (
            Save(raw_text=True, attributes=False),
            {"raw_text": True, "attributes": False},
        ),
    ],
)
def test_all_matches_the_handle_api(save, flags):
    query = Query.all("a", save).build()
    result = run(PAGES, query)

    fields = {"tag": pl.String}
    for name in ("inner_html", "raw_text", "text"):
        if flags.get(name):
            fields[name] = pl.String
    if flags.get("attributes", True):
        fields["attributes"] = PAIRS
    assert result.dtype == pl.List(pl.Struct(fields))

    expected = []
    for html in PAGES:
        elements = matches(html, query, "a")
        expected.append(
            None if elements is None else [handle_row(e, flags) for e in elements]
        )
    assert result.to_list() == expected


def test_named_attributes():
    query = Query.all("a", Save.only_text()).build()
    names = ["href", "id", "class", "disabled", "title", "data-x", "missing"]
    result = run(PAGES, query, attributes=names)
    assert result.dtype == pl.List(
        pl.Struct({n: pl.String for n in ["tag", "text", *names]})
    )

    rows = result.to_list()
    assert rows[0][0]["id"] == "first"
    assert rows[0][0]["class"] == "link external"
    assert rows[0][0]["title"] == ""
    assert rows[0][1]["disabled"] is None  # valueless
    assert rows[0][1]["data-x"] == "12"
    assert rows[0][2]["class"] == ""
    assert all(row["missing"] is None for row in rows[0])
    for html, row in zip(PAGES, rows):
        elements = matches(html, query, "a")
        expected = (
            None
            if elements is None
            else [handle_row(e, {"text": True}, names) for e in elements]
        )
        assert row == expected


def test_valueless_attributes_are_null_pairs():
    query = Query.all("a", Save.none()).build()
    pairs = run(PAGES, query).to_list()[0][1]["attributes"]
    assert pairs == [
        {"name": "href", "value": "/b"},
        {"name": "disabled", "value": None},
        {"name": "data-x", "value": "12"},
    ]


def test_first_is_a_nullable_struct():
    query = Query.first("a[href]", Save.only_text()).build()
    result = run(PAGES, query, attributes=["href"])
    assert result.dtype == pl.Struct(
        {"tag": pl.String, "text": pl.String, "href": pl.String}
    )
    assert result.to_list() == [
        {
            "tag": "a",
            "text": "Fish & chips are great",
            "href": "https://example.com/a/very/long/path",
        },
        None,
        None,
        None,
        {"tag": "a", "text": "only", "href": "/only"},
    ]
    assert result.is_null().to_list() == [False, True, True, True, False]


def test_child_sections_nest_by_selector():
    query = (
        Query.all("div.product", Save.name_only())
        .then(
            lambda product: [
                product.first("h1", Save.only_text()),
                product.all("span.rating", Save.only_text()),
            ]
        )
        .build()
    )
    result = run(CATALOG, query, attributes=["class"])
    child = {"tag": pl.String, "text": pl.String, "class": pl.String}
    assert result.dtype == pl.List(
        pl.Struct(
            {
                "tag": pl.String,
                "h1": pl.Struct(child),
                "span.rating": pl.List(pl.Struct(child)),
            }
        )
    )

    expected = []
    for html in CATALOG:
        products = matches(html, query, "div.product")
        if products is None:
            expected.append(None)
            continue
        rows = []
        for product in products:
            h1 = children(product, "h1")
            spans = children(product, "span.rating")
            rows.append(
                {
                    "tag": product.name,
                    "h1": handle_row(h1[0], {"text": True}, ["class"]) if h1 else None,
                    "span.rating": [
                        handle_row(s, {"text": True}, ["class"]) for s in spans
                    ],
                }
            )
        expected.append(rows)
    assert result.to_list() == expected
    assert result.to_list()[0][2] == {"tag": "div", "h1": None, "span.rating": []}


def children(element, selector):
    try:
        return element.get(selector)
    except ValueError:
        return []


def test_chained_selectors_nest_like_then():
    query = Query.all("ul", Save.name_only()).all("a", Save.name_only()).build()
    result = run(PAGES, query)
    assert result.to_list()[0] == [{"tag": "ul", "a": [{"tag": "a"}] * 4}]


def test_user_example_shape():
    df = pl.DataFrame({"site": ["a", "b"], "html": [PAGES[0], PAGES[4]]})
    query = Query.all("a[href]", Save.all()).build()
    df = df.with_columns(scah.polars.parse(pl.col("html"), query).alias("results"))
    assert df.columns == ["site", "html", "results"]
    links = (
        df.select("site", "results")
        .explode("results")
        .with_columns(
            pl.col("results")
            .struct.field("attributes")
            .list.eval(
                pl.element()
                .filter(pl.element().struct.field("name") == "href")
                .struct.field("value")
            )
            .list.first()
            .alias("href")
        )
        .select("site", "href")
    )
    assert links.rows() == [
        ("a", "https://example.com/a/very/long/path"),
        ("a", "/b"),
        ("a", "/é"),
        ("b", "/only"),
    ]


# ── Execution ──────────────────────────────────────────────────────────────


def test_lazy_schema_matches_the_result():
    query = (
        Query.all("div.product", Save.all())
        .then(lambda p: [p.first("h1", Save.only_text())])
        .build()
    )
    lf = frame(CATALOG).lazy().with_columns(parse("html", query).alias("r"))
    schema = lf.collect_schema()
    assert lf.collect().schema == schema


@pytest.mark.parametrize("engine", ["in-memory", "streaming"])
def test_engines_agree_across_chunks(engine):
    query = Query.all("a", Save.all()).build()
    pages = PAGES * 200
    chunked = pl.concat(
        [frame(pages[i : i + 37]) for i in range(0, len(pages), 37)], rechunk=False
    )
    assert chunked["html"].n_chunks() > 1
    out = chunked.lazy().select(parse("html", query).alias("r")).collect(engine=engine)
    assert out["r"].to_list() == run(PAGES, query).to_list() * 200


def test_empty_frame_keeps_the_schema():
    query = Query.all("a", Save.only_text()).build()
    result = run([], query, attributes=["href"])
    assert len(result) == 0
    assert result.dtype == pl.List(
        pl.Struct({"tag": pl.String, "text": pl.String, "href": pl.String})
    )


def test_query_is_reusable_across_expressions():
    query = Query.all("a", Save.only_text()).build()
    df = frame(PAGES).with_columns(
        parse("html", query, attributes=["href"]).alias("one"),
        parse("html", query).alias("two"),
    )
    assert (
        df["one"].list.len().to_list()
        == df["two"].list.len().to_list()
        == [4, None, 0, 0, 1]
    )


# ── Errors ─────────────────────────────────────────────────────────────────


def test_rejects_non_string_input():
    query = Query.all("a", Save.none()).build()
    with pytest.raises(pl.exceptions.ComputeError, match="String column"):
        pl.DataFrame({"html": [1, 2]}).select(parse("html", query))


def test_rejects_duplicate_field_names():
    query = (
        Query.all("svg", Save.only_text())
        .then(lambda svg: [svg.all("text", Save.none())])
        .build()
    )
    with pytest.raises(pl.exceptions.ComputeError, match="two fields named `text`"):
        frame(["<svg><text>x</text></svg>"]).select(parse("html", query))
    with pytest.raises(pl.exceptions.ComputeError, match="two fields named `tag`"):
        frame(["<a></a>"]).select(
            parse("html", Query.all("a", Save.none()).build(), attributes=["tag"])
        )


def test_rejects_a_string_for_attributes():
    with pytest.raises(TypeError):
        parse("html", Query.all("a", Save.none()).build(), attributes="href")


def test_import_scah_does_not_import_polars_eagerly():
    import subprocess
    import sys

    code = "import sys, scah; assert 'polars' not in sys.modules; scah.polars; assert 'polars' in sys.modules"
    subprocess.run([sys.executable, "-c", code], check=True)


# ── Threads ────────────────────────────────────────────────────────────────

PAGE = "<div>" + "<a href='/x'>x &amp; y</a>" * 3000 + "</div>"  # ~75 KB


def test_large_chunks_split_across_threads():
    """Over 1 MiB of HTML in one chunk runs on several threads, one output
    chunk each; the values don't depend on the split."""
    pages = [PAGE if i % 7 else None for i in range(100)]
    query = Query.all("a", Save.only_text()).build()
    result = run(pages, query, attributes=["href"])
    row = [{"tag": "a", "text": "x & y", "href": "/x"}] * 3000
    assert result.to_list() == [row if i % 7 else None for i in range(100)]
