"""CSS selectors meet streaming HTML parsing: extract exactly what you select
without building a full DOM.

`scah.polars` runs queries on a column of HTML inside polars; it is imported on
first use, so `import scah` doesn't import polars.
"""

from scah._scah import (
    ArrowTable,
    Element,
    PyQuery,
    PyQueryBuilder,
    PyQueryFactory,
    Query,
    Save,
    Store,
    parse,
)

__all__ = [
    "ArrowTable",
    "Element",
    "PyQuery",
    "PyQueryBuilder",
    "PyQueryFactory",
    "Query",
    "Save",
    "Store",
    "parse",
]


def __getattr__(name: str):
    if name == "polars":
        import importlib

        return importlib.import_module("scah.polars")
    raise AttributeError(f"module 'scah' has no attribute {name!r}")
