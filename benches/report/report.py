"""Normalize benchmark output into benches/results and render README tables.

Every suite (rust, python, node) is stored in one format so the README and
the project website read the same data:

    benches/results/<suite>/<scenario>.json

Usage:
    report.py import-criterion criterion.json
    report.py import-pytest whatwg.json --suite python --scenario whatwg-all-links
    report.py readme README.md crates/bindings/scah-python/README.md

The pytest importer also accepts the pytest-benchmark-shaped JSON written by
the Node benchmark (crates/bindings/scah-node/benchmark/bench.ts).
"""

from __future__ import annotations

import argparse
import json
import platform
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

SCHEMA_VERSION = 1
REPO_ROOT = Path(__file__).resolve().parents[2]
RESULTS_DIR = REPO_ROOT / "benches" / "results"
SUITES = ("rust", "python", "node")

# Display order of README columns.
SCENARIOS = {
    "whatwg-all-links": {
        "label": "WHATWG spec",
        "title": "Every <a> in the WHATWG HTML specification",
    },
    "nested-all": {
        "label": "Nested (all)",
        "title": "Every product card, with nested title, rating, and description queries",
    },
    "nested-first": {
        "label": "Nested (first)",
        "title": "The first product card, with nested title, rating, and description queries",
    },
    "simple-all": {
        "label": "Flat (all)",
        "title": "Every <a> in a flat list of <div><a> pairs",
    },
    "simple-first": {
        "label": "Flat (first)",
        "title": "The first <a> in a flat list of <div><a> pairs",
    },
}

CRITERION_GROUPS = {
    "simple_all_selection_comparison": "simple-all",
    "simple_first_selection_comparison": "simple-first",
    "nested_all_selection_comparison": "nested-all",
    "nested_first_selection_comparison": "nested-first",
    "whatwg_html_spec_all_links": "whatwg-all-links",
}

# The comparison groups also hold internal scah measurements, such as
# scah_query_build_only and scah_parse_prebuilt_*, that are not libraries.
CRITERION_LIBRARIES = {"scah", "lol_html", "tl", "lexbor", "scraper", "lxml"}

MARKER = re.compile(
    r"(<!-- benchmarks:(?P<suite>[a-z]+) -->\n).*?(<!-- /benchmarks:(?P=suite) -->)",
    re.DOTALL,
)


# --- environment ---------------------------------------------------------


def _run(*args: str) -> str | None:
    try:
        return subprocess.run(
            args, capture_output=True, text=True, check=True, cwd=REPO_ROOT
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return None


def _cpu() -> str | None:
    cpuinfo = Path("/proc/cpuinfo")
    if cpuinfo.exists():
        for line in cpuinfo.read_text().splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    return _run("sysctl", "-n", "machdep.cpu.brand_string") or platform.processor() or None


def capture_environment(runtime: str | None) -> dict:
    commit = _run("git", "rev-parse", "HEAD")
    dirty = _run("git", "status", "--porcelain", "--untracked-files=no")
    return {
        "date": datetime.now(timezone.utc).date().isoformat(),
        "commit": commit,
        "dirty": bool(dirty) if commit else None,
        "os": f"{platform.system()} {platform.release()}",
        "arch": platform.machine(),
        "cpu": _cpu(),
        "runtime": runtime,
    }


# --- results files -------------------------------------------------------


def result_path(suite: str, scenario: str) -> Path:
    return RESULTS_DIR / suite / f"{scenario}.json"


def write_result(suite: str, scenario: str, environment: dict, results: list[dict]) -> Path:
    if scenario not in SCENARIOS:
        raise SystemExit(f"unknown scenario {scenario!r}; expected one of {list(SCENARIOS)}")
    document = {
        "schema_version": SCHEMA_VERSION,
        "suite": suite,
        "scenario": scenario,
        "title": SCENARIOS[scenario]["title"],
        "unit": "ms",
        "environment": environment,
        "results": sorted(results, key=lambda r: (r["size"] or 0, r["mean"])),
    }
    path = result_path(suite, scenario)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, indent=2, ensure_ascii=False) + "\n")
    print(f"wrote {path}")
    return path


def load_results(suite: str) -> dict[str, dict]:
    loaded = {}
    for scenario in SCENARIOS:
        path = result_path(suite, scenario)
        if path.exists():
            loaded[scenario] = json.loads(path.read_text())
    return loaded


# --- importers -----------------------------------------------------------


def import_criterion(path: Path) -> list[Path]:
    """Read `cargo criterion --message-format=json` output.

    Later records win, so appending several runs to one file is safe.
    """
    latest: dict[str, dict] = {}
    for line in path.read_text().splitlines():
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        if record.get("reason") == "benchmark-complete":
            latest[record["id"]] = record

    by_scenario: dict[str, list[dict]] = {}
    for bench_id, record in latest.items():
        group, library, *rest = bench_id.split("/")
        scenario = CRITERION_GROUPS.get(group)
        if scenario is None or library not in CRITERION_LIBRARIES:
            continue
        per_iteration = [
            measured / count
            for measured, count in zip(record["measured_values"], record["iteration_count"])
            if count
        ]
        by_scenario.setdefault(scenario, []).append(
            {
                "library": library,
                "size": int(rest[0]) if rest and rest[0].isdigit() else None,
                "mean": record["mean"]["estimate"] / 1e6,
                "stdev": _stdev(per_iteration) / 1e6,
            }
        )

    environment = capture_environment(_run("rustc", "--version"))
    return [
        write_result("rust", scenario, environment, results)
        for scenario, results in by_scenario.items()
    ]


def import_pytest(
    path: Path, suite: str, scenario: str, runtime: str | None, size: int | None
) -> Path:
    data = json.loads(path.read_text())
    machine = data.get("machine_info", {})
    if runtime is None and "python_version" in machine:
        runtime = f"{machine.get('python_implementation', 'Python')} {machine['python_version']}"
    results = [
        {
            "library": bench["name"],
            "size": size,
            "mean": bench["stats"]["mean"] * 1e3,
            "stdev": bench["stats"]["stddev"] * 1e3,
        }
        for bench in data["benchmarks"]
    ]
    return write_result(suite, scenario, capture_environment(runtime), results)


def _stdev(values: list[float]) -> float:
    if len(values) < 2:
        return 0.0
    mean = sum(values) / len(values)
    return (sum((v - mean) ** 2 for v in values) / (len(values) - 1)) ** 0.5


# --- README tables -------------------------------------------------------


def format_duration(ms: float) -> str:
    for scale, unit in ((1e3, "s"), (1.0, "ms"), (1e-3, "µs"), (1e-6, "ns")):
        if ms >= scale or unit == "ns":
            value = ms / scale
            digits = f"{value:#.3g}".rstrip(".") if value < 1000 else f"{value:,.0f}"
            return f"{digits} {unit}"
    raise AssertionError("unreachable")


def format_ratio(ratio: float) -> str:
    if ratio >= 100:
        return f"{ratio:,.0f}×"
    if ratio >= 10:
        return f"{ratio:.1f}×"
    return f"{ratio:.2f}×"


def _headline(document: dict) -> dict[str, float]:
    """Mean per library at the largest input size."""
    sizes = [r["size"] or 0 for r in document["results"]]
    largest = max(sizes, default=0)
    return {
        r["library"]: r["mean"]
        for r in document["results"]
        if (r["size"] or 0) == largest
    }


def _baseline(means: dict[str, float]) -> str | None:
    return next((name for name in means if name.lower() == "scah"), None)


def render_table(suite: str) -> str:
    documents = load_results(suite)
    if not documents:
        raise SystemExit(f"no results for suite {suite!r} in {RESULTS_DIR}")

    columns = [(scenario, _headline(doc)) for scenario, doc in documents.items()]

    libraries: list[str] = []
    for _, means in columns:
        for library in sorted(means, key=means.get):
            if library not in libraries:
                libraries.append(library)
    baseline = next((l for l in libraries if l.lower() == "scah"), None)
    if baseline:
        libraries.remove(baseline)
        libraries.insert(0, baseline)

    header = ["Library"] + [SCENARIOS[s]["label"] for s, _ in columns]
    rows = [header, [":---"] + ["---:"] * len(columns)]
    for library in libraries:
        cells = [f"**{library}**" if library == baseline else library]
        for _, means in columns:
            mean = means.get(library)
            if mean is None:
                cells.append("—")
                continue
            cell = format_duration(mean)
            base = means.get(_baseline(means) or "")
            if base and library != baseline:
                cell += f" ({format_ratio(mean / base)})"
            cells.append(f"**{cell}**" if library == baseline else cell)
        rows.append(cells)

    sizes = sorted({max((r["size"] or 0) for r in doc["results"]) for doc in documents.values()} - {0})
    dates = sorted({doc["environment"]["date"] for doc in documents.values()})
    notes = ["Mean time per parse and query; lower is better. Multipliers are relative to scah."]
    if sizes:
        notes.append(f"Synthetic inputs use {', '.join(f'{s:,}' for s in sizes)} elements.")
    notes.append(
        f"Measured {' to '.join(dict.fromkeys([dates[0], dates[-1]]))}; "
        f"raw data and run details in [`benches/results/{suite}`]"
        f"(https://github.com/zacharyvmm/scah/tree/main/benches/results/{suite})."
    )

    table = "\n".join("| " + " | ".join(row) + " |" for row in rows)
    return f"{table}\n\n{' '.join(notes)}\n"


def render_readme(text: str) -> str:
    return MARKER.sub(
        lambda m: f"{m.group(1)}{render_table(m.group('suite'))}{m.group(3)}", text
    )


# --- cli -----------------------------------------------------------------


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)

    criterion = commands.add_parser("import-criterion", help="import cargo-criterion JSON")
    criterion.add_argument("input", type=Path)

    pytest = commands.add_parser("import-pytest", help="import pytest-benchmark JSON")
    pytest.add_argument("input", type=Path)
    pytest.add_argument("--suite", choices=("python", "node"), required=True)
    pytest.add_argument("--scenario", choices=list(SCENARIOS), required=True)
    pytest.add_argument("--size", type=int, help="input element count for synthetic scenarios")
    pytest.add_argument("--runtime", help='e.g. "bun 1.3.0"; defaults to the Python version in the input')

    readme = commands.add_parser("readme", help="regenerate <!-- benchmarks:SUITE --> blocks")
    readme.add_argument("files", type=Path, nargs="+")
    readme.add_argument("--check", action="store_true", help="fail if any file is out of date")

    args = parser.parse_args(argv)
    if args.command == "import-criterion":
        import_criterion(args.input)
    elif args.command == "import-pytest":
        import_pytest(args.input, args.suite, args.scenario, args.runtime, args.size)
    elif args.command == "readme":
        stale = []
        for file in args.files:
            original = file.read_text()
            updated = render_readme(original)
            if updated == original:
                continue
            stale.append(str(file))
            if not args.check:
                file.write_text(updated)
                print(f"updated {file}")
        if args.check and stale:
            print("benchmark tables are out of date; run `just bench-readme`:", *stale, sep="\n  ")
            return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
