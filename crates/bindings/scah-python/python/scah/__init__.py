"""CSS selectors meet streaming HTML parsing: extract exactly what you select
without building a full DOM.

`scah.polars` runs queries on a column of HTML inside polars; it is imported on
first use, so `import scah` doesn't import polars.
"""

from scah._scah import (
    ArrowTable,
    CompiledQuery,
    Element,
    Query,
    QueryBuilder,
    QueryFactory,
    Save,
    Store,
    parse,
)

__all__ = [
    "ArrowTable",
    "CompiledQuery",
    "Element",
    "Query",
    "QueryBuilder",
    "QueryFactory",
    "Save",
    "Store",
    "parse",
]

# The names these classes had before 0.2.0.
_DEPRECATED_NAMES = {
    "PyQuery": "CompiledQuery",
    "PyQueryBuilder": "QueryBuilder",
    "PyQueryFactory": "QueryFactory",
}


def __getattr__(name: str):
    if name == "polars":
        import importlib

        return importlib.import_module("scah.polars")
    if name in _DEPRECATED_NAMES:
        import warnings

        new_name = _DEPRECATED_NAMES[name]
        warnings.warn(
            f"scah.{name} is deprecated; use scah.{new_name}",
            DeprecationWarning,
            stacklevel=2,
        )
        return globals()[new_name]
    raise AttributeError(f"module 'scah' has no attribute {name!r}")
