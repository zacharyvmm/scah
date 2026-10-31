"""Run scah queries on a column of HTML documents inside polars.

```python
import polars as pl
import scah

query = scah.Query.all("a[href]", scah.Save.only_text()).build()

pages.with_columns(
    scah.polars.parse(pl.col("html"), query, attributes=["href"]).alias("links")
)
```

`parse` is a polars expression plugin: the query runs in Rust on polars'
threads, without the GIL, and works in lazy and streaming queries, so
`pl.scan_parquet(...).with_columns(...).sink_parquet(...)` scrapes a dataset
larger than memory.
"""

from __future__ import annotations

from collections.abc import Sequence
from pathlib import Path
from typing import TYPE_CHECKING

from polars.plugins import register_plugin_function

from scah import _scah

if TYPE_CHECKING:
    import polars as pl
    from polars._typing import IntoExpr

    from scah import PyQuery

__all__ = ["parse"]

_PLUGIN = Path(_scah.__file__)


def parse(
    html: IntoExpr,
    query: PyQuery,
    *,
    attributes: Sequence[str] | None = None,
) -> pl.Expr:
    """Run `query` on each HTML document in the String column `html`.

    The result has one value per document, shaped by the query:

    - `Query.all(...)` gives a `list[struct]` of every match, in document
      order, and `Query.first(...)` a `struct` of the first match, or null.
    - Each struct has the match's `tag`, then the content its `Save` keeps:
      `inner_html`, `raw_text`, `text`.
    - If the `Save` keeps attributes, `attributes=None` adds an `attributes`
      field listing every attribute as a `{name, value}` struct (`class`
      first, then `id`, then the rest in source order). A list of names adds
      one String field per name instead, null where the match lacks it.
    - Each child section of the query (from `.then(...)`) adds a field named
      by its selector, shaped the same way.

    A null document gives a null value. An invalid selector or a field name
    used twice (an attribute named like a child selector, say) raises when
    polars resolves the schema.
    """
    if isinstance(attributes, str):
        msg = "attributes must be a sequence of names, not a string"
        raise TypeError(msg)
    return register_plugin_function(
        plugin_path=_PLUGIN,
        function_name="parse",
        args=html,
        kwargs={
            "sections": query._sections(),
            "attributes": None if attributes is None else list(attributes),
        },
        is_elementwise=True,
    )
