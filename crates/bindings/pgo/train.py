"""Train scah's Python binding for profile-guided optimization.

build.sh runs this against an instrumented build of the binding. It parses
page.html, and a large document made of its repeated content, with each query
in queries.json alone and with all of them at once. Then it reads every field
of every result.
"""

import json
from pathlib import Path

from scah import Query, Save, parse

HERE = Path(__file__).resolve().parent
SMALL_ROUNDS = 150
LARGE_ROUNDS = 4
BUILD_ROUNDS = 100


def save(spec):
    return Save(
        inner_html=spec.get("inner_html", False),
        text=spec.get("text", False),
        attributes=spec.get("attributes", True),
        raw_text=spec.get("raw_text", False),
    )


def selector(spec):
    return spec.get("all") or spec["first"]


def builder(spec, factory=Query):
    start = factory.all if "all" in spec else factory.first
    query = start(selector(spec), save(spec["save"]))
    if "then" in spec:
        query = query.then(lambda child: [builder(c, child) for c in spec["then"]])
    return query


def read(elements, spec):
    for element in elements or []:
        element.name, element.id, element.class_name, element.attributes
        element.get_attribute("href"), element["text"]
        element.inner_html, element.raw_text, element.text
        for child in spec.get("then", []):
            try:
                read(element.get(selector(child)), child)
            except ValueError:
                pass


def run(html, queries, specs):
    for query, spec in zip(queries, specs):
        read(parse(html, [query]).query(0), spec)
    store = parse(html, queries)
    for index, spec in enumerate(specs):
        read(store.query(index), spec)
        read(store.get(selector(spec)), spec)


def large_document(page):
    head, rest = page.split('<main id="content">')
    content, tail = rest.split("</main>")
    return f'{head}<main id="content">{content * 50}</main>{tail}'


def main():
    page = (HERE / "page.html").read_text()
    specs = json.loads((HERE / "queries.json").read_text())
    queries = [builder(spec).build() for spec in specs]

    for _ in range(BUILD_ROUNDS):
        [builder(spec).build() for spec in specs]
    for _ in range(SMALL_ROUNDS):
        run(page, queries, specs)
    large = large_document(page)
    for _ in range(LARGE_ROUNDS):
        run(large, queries, specs)


if __name__ == "__main__":
    main()
