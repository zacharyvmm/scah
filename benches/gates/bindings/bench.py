"""Time scah's Python binding on fixed workloads and print the results as JSON.

check-binding-performance.sh runs this script against a base and a candidate
build of the binding. Every workload uses only API that both builds share.
"""

import gc
import json
import sys
import time

from scah import Query, Save, parse

SAMPLES = 20


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


def product_query():
    return (
        Query.all("div.product", Save.none())
        .then(
            lambda product: [
                product.first("h1", Save.only_text()),
                product.first("span.rating", Save.only_text()),
                product.first("p.description", Save(inner_html=True)),
            ]
        )
        .build()
    )


def workloads():
    links = links_html(10_000)
    catalog = catalog_html(2_000)
    link_query = Query.all("a", Save(inner_html=True, text=True)).build()
    first_query = Query.first("a", Save(inner_html=True, text=True)).build()
    nested_query = product_query()

    store = parse(links, [link_query])
    anchors = store.get("a")
    products = parse(catalog, [nested_query]).get("div.product")

    def parse_all():
        parse(links, [link_query]).get("a")

    def parse_first():
        for _ in range(100):
            parse(links, [first_query]).get("a")

    def parse_nested():
        for product in parse(catalog, [nested_query]).get("div.product"):
            product.get("h1")[0].text
            product.get("span.rating")[0].text

    def store_get():
        for _ in range(10):
            store.get("a")

    def element_fields():
        for anchor in anchors:
            anchor.name
            anchor.id
            anchor.class_name
            anchor.get_attribute("href")
            anchor.inner_html
            anchor.text

    def element_attributes():
        for anchor in anchors:
            anchor.attributes

    def element_get():
        for product in products:
            product.get("h1")
            product.get("span.rating")

    def build_query():
        for _ in range(1_000):
            product_query()

    return {
        "parse_all": parse_all,
        "parse_first": parse_first,
        "parse_nested": parse_nested,
        "store_get": store_get,
        "element_fields": element_fields,
        "element_attributes": element_attributes,
        "element_get": element_get,
        "build_query": build_query,
    }


def measure(workload):
    """Return the fastest of several timed runs, in nanoseconds."""
    for _ in range(3):
        workload()
    gc.collect()
    gc.disable()
    try:
        best = None
        for _ in range(SAMPLES):
            start = time.perf_counter_ns()
            workload()
            elapsed = time.perf_counter_ns() - start
            best = elapsed if best is None else min(best, elapsed)
        return best
    finally:
        gc.enable()


def main():
    results = {name: measure(workload) for name, workload in workloads().items()}
    json.dump(results, sys.stdout)


if __name__ == "__main__":
    main()
