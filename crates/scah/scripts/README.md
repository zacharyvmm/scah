# HTML entity table generation

`generate_entities.py` turns `entities.json`, a committed copy of the
[WHATWG named character reference dataset](https://html.spec.whatwg.org/entities.json),
into `src/html/entities_table.rs`. This directory is excluded from the
published crate; only the generated table ships.

* Source SHA-256: `d741d877ac77c4194c4ad526b5b4a19aef8dfe411ab840a466891cdbb9f362e6`
* Entry count: 2231
* License: [`THIRD_PARTY_LICENSES/WHATWG-HTML.txt`](../THIRD_PARTY_LICENSES/WHATWG-HTML.txt)

```bash
# Regenerate the table from the committed fixture (offline, deterministic)
python crates/scah/scripts/generate_entities.py

# Download the latest upstream dataset and regenerate
python crates/scah/scripts/generate_entities.py --update-fixture
```

After updating from upstream, record the new SHA-256 and entry count here and
in the license notice, then commit the fixture, table, and notice together.
CI fails if any of them disagree.
