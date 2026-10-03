"""Compare the element handle API with Arrow export in scah's Python binding.

Every workload runs in one process against one build, so the numbers compare
approaches rather than revisions. Each is timed as the fastest of several
runs with the garbage collector disabled, once on a store parsed ahead of
time and once including the parse.

Run against a release build of the binding, for example:

    cd crates/bindings/scah-python
    uv venv /tmp/scah-arrow-bench
    VIRTUAL_ENV=/tmp/scah-arrow-bench uvx maturin develop --release
    uv pip install --python /tmp/scah-arrow-bench pyarrow polars
    /tmp/scah-arrow-bench/bin/python ../../../benches/experiments/arrow/bench.py
"""

import argparse
import gc
import platform
import sys
import time

import polars as pl
import pyarrow as pa

from scah import Query, Save, parse

LINK_SIZES = (1_000, 10_000, 100_000)
CATALOG_SIZE = 10_000
ATTRIBUTES = ["id", "class", "href"]


def links_html(count):
    items = "".join(
        f'<div class="article"><a id="post-{i}" class="link" href="/post/{i}">'
        f"<b>Post</b> &lt;{i}&gt;</a></div>"
        for i in range(count)
    )
    return f"<html><body><div id='content'>{items}</div></body></html>"


def catalog_html(count):
    items = "".join(
        f'<div class="product"><h1>Product #{i}</h1>'
        f'<span class="rating">{i % 5 + 1}/5</span>'
        f'<p class="description">Description</p></div>'
        for i in range(count)
    )
    return f'<html><body><section id="products">{items}</section></body></html>'


def link_query():
    return Query.all("a", Save(inner_html=True, text=True)).build()


def product_query():
    return (
        Query.all("div.product", Save.none())
        .then(
            lambda product: [
                product.first("h1", Save.only_text()),
                product.first("span.rating", Save.only_text()),
            ]
        )
        .build()
    )


# ── Link workloads: each takes a parsed store ─────────────────────────────


def handles_rows(store):
    return [
        (a.name, a.id, a.class_name, a.get_attribute("href"), a.inner_html, a.text)
        for a in store.get("a")
    ]


def handles_polars(store):
    columns = {
        "tag": [],
        "id": [],
        "class": [],
        "href": [],
        "inner_html": [],
        "text": [],
    }
    for a in store.get("a"):
        columns["tag"].append(a.name)
        columns["id"].append(a.id)
        columns["class"].append(a.class_name)
        columns["href"].append(a.get_attribute("href"))
        columns["inner_html"].append(a.inner_html)
        columns["text"].append(a.text)
    return pl.DataFrame(columns)


def arrow_export(store):
    schema, array = store.to_arrow("a", attributes=ATTRIBUTES).__arrow_c_array__()
    # Dropping the unconsumed capsules releases the exported structs.
    del schema, array


def arrow_pyarrow(store):
    return pa.record_batch(store.to_arrow("a", attributes=ATTRIBUTES))


def arrow_polars(store):
    return pl.DataFrame(store.to_arrow("a", attributes=ATTRIBUTES))


def arrow_to_pylist(store):
    return pa.record_batch(store.to_arrow("a", attributes=ATTRIBUTES)).to_pylist()


LINK_WORKLOADS = {
    "handles_rows": handles_rows,
    "handles_polars": handles_polars,
    "arrow_export": arrow_export,
    "arrow_pyarrow": arrow_pyarrow,
    "arrow_polars": arrow_polars,
    "arrow_to_pylist": arrow_to_pylist,
}


# ── Nested workloads ──────────────────────────────────────────────────────


def handles_nested_polars(store):
    titles, ratings = [], []
    for product in store.get("div.product"):
        titles.append(product.get("h1")[0].text)
        ratings.append(product.get("span.rating")[0].text)
    return pl.DataFrame({"title": titles, "rating": ratings})


def arrow_nested_polars(store):
    products = pl.DataFrame(store.to_arrow("div.product")).select("index")
    titles = pl.DataFrame(store.to_arrow("h1", parent="div.product")).select(
        "parent", title="text"
    )
    ratings = pl.DataFrame(store.to_arrow("span.rating", parent="div.product")).select(
        "parent", rating="text"
    )
    join = {
        "left_on": "index",
        "right_on": "parent",
        "how": "left",
        "maintain_order": "left",
    }
    return (
        products.join(titles, **join).join(ratings, **join).select("title", "rating")
    )


NESTED_WORKLOADS = {
    "handles_nested_polars": handles_nested_polars,
    "arrow_nested_polars": arrow_nested_polars,
}


# ── Timing ────────────────────────────────────────────────────────────────


def measure(workload, samples):
    """Return the fastest of `samples` timed runs, in milliseconds."""
    for _ in range(2):
        workload()
    gc.collect()
    gc.disable()
    try:
        best = None
        for _ in range(samples):
            start = time.perf_counter_ns()
            workload()
            elapsed = time.perf_counter_ns() - start
            best = elapsed if best is None else min(best, elapsed)
        return best / 1e6
    finally:
        gc.enable()


def samples_for(count, override):
    if override:
        return override
    return 10 if count >= 100_000 else 20


def check_equivalence():
    """Fail fast if the two approaches disagree on the data."""
    store = parse(links_html(100), [link_query()])
    from_handles = handles_polars(store)
    from_arrow = arrow_polars(store).select(from_handles.columns)
    assert from_arrow.equals(from_handles), "link tables differ"

    store = parse(catalog_html(100), [product_query()])
    assert arrow_nested_polars(store).equals(handles_nested_polars(store)), (
        "nested tables differ"
    )


def run(workloads, html, query, samples):
    """Time each workload on a parsed store and including the parse."""
    store = parse(html, [query])
    results = {"parse_only": (None, measure(lambda: parse(html, [query]), samples))}
    for name, workload in workloads.items():
        parsed = measure(lambda: workload(store), samples)
        with_parse = measure(lambda: workload(parse(html, [query])), samples)
        results[name] = (parsed, with_parse)
    return results


def print_table(title, sizes, results, column):
    names = list(next(iter(results.values())))
    header = ["workload", *(f"{size:,}" for size in sizes)]
    rows = []
    for name in names:
        cells = []
        for size in sizes:
            value = results[size][name][column]
            cells.append("-" if value is None else f"{value:.3f}")
        rows.append([name, *cells])
    widths = [max(len(row[i]) for row in [header, *rows]) for i in range(len(header))]

    print(f"\n{title}\n")
    print("| " + " | ".join(h.ljust(w) for h, w in zip(header, widths)) + " |")
    print("|" + "|".join("-" * (w + 2) for w in widths) + "|")
    for row in rows:
        cells = [row[0].ljust(widths[0])] + [
            cell.rjust(width) for cell, width in zip(row[1:], widths[1:])
        ]
        print("| " + " | ".join(cells) + " |")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--samples", type=int, help="timed runs per workload")
    parser.add_argument(
        "--sizes",
        type=lambda text: tuple(int(size) for size in text.split(",")),
        default=LINK_SIZES,
        help="comma-separated link counts",
    )
    args = parser.parse_args()

    check_equivalence()
    print(
        f"Python {platform.python_version()}, pyarrow {pa.__version__}, "
        f"polars {pl.__version__}, {platform.platform()}"
    )
    print("Times are the minimum over runs, in milliseconds.")

    link_results = {}
    for count in args.sizes:
        print(f"links: {count:,} elements...", file=sys.stderr)
        link_results[count] = run(
            LINK_WORKLOADS,
            links_html(count),
            link_query(),
            samples_for(count, args.samples),
        )
    print_table("Links, store already parsed", args.sizes, link_results, 0)
    print_table("Links, including parse", args.sizes, link_results, 1)

    print(f"catalog: {CATALOG_SIZE:,} products...", file=sys.stderr)
    nested = {
        CATALOG_SIZE: run(
            NESTED_WORKLOADS,
            catalog_html(CATALOG_SIZE),
            product_query(),
            samples_for(CATALOG_SIZE, args.samples),
        )
    }
    print_table("Nested catalog, store already parsed", [CATALOG_SIZE], nested, 0)
    print_table("Nested catalog, including parse", [CATALOG_SIZE], nested, 1)


if __name__ == "__main__":
    main()
