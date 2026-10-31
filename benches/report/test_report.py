import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).parent))
import report  # noqa: E402


def criterion_line(bench_id, mean_ns):
    return json.dumps(
        {
            "reason": "benchmark-complete",
            "id": bench_id,
            "measured_values": [mean_ns * 10, mean_ns * 20],
            "iteration_count": [10, 20],
            "mean": {"estimate": mean_ns},
        }
    )


class ReportTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        patcher = mock.patch.object(report, "RESULTS_DIR", Path(self.tmp.name))
        patcher.start()
        self.addCleanup(patcher.stop)

    def test_format_duration_picks_unit_and_keeps_three_figures(self):
        self.assertEqual(report.format_duration(0.000408), "408 ns")
        self.assertEqual(report.format_duration(0.0472), "47.2 µs")
        self.assertEqual(report.format_duration(2.4), "2.40 ms")
        self.assertEqual(report.format_duration(968.0), "968 ms")
        self.assertEqual(report.format_duration(5449.6), "5.45 s")

    def test_format_ratio(self):
        self.assertEqual(report.format_ratio(0.957), "0.96×")
        self.assertEqual(report.format_ratio(15.55), "15.6×")
        self.assertEqual(report.format_ratio(4139.6), "4,140×")

    def test_import_criterion_keeps_latest_record_and_comparison_libraries(self):
        source = Path(self.tmp.name) / "criterion.json"
        source.write_text(
            "\n".join(
                [
                    criterion_line("simple_all_selection_comparison/scah/100", 2_000_000),
                    criterion_line("simple_all_selection_comparison/scah/100", 1_000_000),
                    criterion_line("simple_all_selection_comparison/tl/100", 3_000_000),
                    criterion_line("simple_all_selection_comparison/scah_query_build_only/100", 5),
                    criterion_line("cursor_domination/descendant/parse/8", 1),
                    "not json",
                ]
            )
        )
        with mock.patch.object(report, "capture_environment", return_value={"date": "2026-10-01"}):
            written = report.import_criterion(source)

        self.assertEqual([p.name for p in written], ["simple-all.json"])
        document = json.loads(written[0].read_text())
        self.assertEqual(document["schema_version"], report.SCHEMA_VERSION)
        self.assertEqual(
            [(r["library"], r["size"], r["mean"]) for r in document["results"]],
            [("scah", 100, 1.0), ("tl", 100, 3.0)],
        )

    def test_import_jmh_splits_methods_into_scenarios(self):
        def run(method, library, score, raw):
            return {
                "benchmark": f"io.github.zacharyvmm.scah.bench.ScrapeBenchmark.{method}",
                "jdkVersion": "25",
                "vmName": "OpenJDK 64-Bit Server VM",
                "params": {"library": library},
                "primaryMetric": {"score": score, "scoreUnit": "ms/op", "rawData": raw},
            }

        source = Path(self.tmp.name) / "jmh.json"
        source.write_text(
            json.dumps(
                [
                    run("simpleAll", "jsoup", 4.0, [[3.0, 5.0], [4.0, 4.0]]),
                    run("simpleAll", "scah (JNI)", 1.0, [[1.0, 1.0]]),
                    run("whatwgAllLinks", "scah", 2.0, [[2.0]]),
                ]
            )
        )
        with mock.patch.object(report, "capture_environment", side_effect=lambda r: {"runtime": r}):
            written = report.import_jmh(source, size=100)

        self.assertEqual(sorted(p.name for p in written), ["simple-all.json", "whatwg-all-links.json"])
        simple = json.loads((Path(self.tmp.name) / "java" / "simple-all.json").read_text())
        self.assertEqual(simple["environment"]["runtime"], "Java 25 (OpenJDK 64-Bit Server VM)")
        self.assertEqual(
            [(r["library"], r["size"], r["mean"]) for r in simple["results"]],
            [("scah (JNI)", 100, 1.0), ("jsoup", 100, 4.0)],
        )
        self.assertAlmostEqual(simple["results"][1]["stdev"], (2 / 3) ** 0.5)
        whatwg = json.loads((Path(self.tmp.name) / "java" / "whatwg-all-links.json").read_text())
        self.assertIsNone(whatwg["results"][0]["size"])

    def test_import_benchmarkdotnet_reads_library_parameter_and_converts_ns(self):
        def bench(method, library, mean_ns):
            return {
                "Method": method,
                "Parameters": f"Library={library}",
                "Statistics": {"Mean": mean_ns, "StandardDeviation": mean_ns / 10},
            }

        source = Path(self.tmp.name) / "report-full.json"
        source.write_text(
            json.dumps(
                {
                    "HostEnvironmentInfo": {"RuntimeVersion": ".NET 10.0.14 (10.0.14, 10.0.1426.52326)"},
                    "Benchmarks": [
                        bench("NestedAll", "HtmlAgilityPack + Fizzler", 3_000_000),
                        bench("NestedAll", "scah", 1_000_000),
                    ],
                }
            )
        )
        with mock.patch.object(report, "capture_environment", side_effect=lambda r: {"runtime": r}):
            written = report.import_benchmarkdotnet(source, size=100)

        document = json.loads(written[0].read_text())
        self.assertEqual(written[0].parent.name, "csharp")
        self.assertEqual(document["scenario"], "nested-all")
        self.assertEqual(document["environment"]["runtime"], ".NET 10.0.14")
        self.assertEqual(
            [(r["library"], r["mean"], r["stdev"]) for r in document["results"]],
            [("scah", 1.0, 0.1), ("HtmlAgilityPack + Fizzler", 3.0, 0.3)],
        )

    def test_readme_block_is_regenerated_from_results(self):
        report.write_result(
            "python",
            "simple-all",
            {"date": "2026-10-01"},
            [
                {"library": "Other", "size": 10000, "mean": 5.0, "stdev": 0.1},
                {"library": "Scah", "size": 10000, "mean": 2.0, "stdev": 0.1},
            ],
        )
        text = "intro\n<!-- benchmarks:python -->\nstale\n<!-- /benchmarks:python -->\noutro\n"
        rendered = report.render_readme(text)

        self.assertNotIn("stale", rendered)
        self.assertTrue(rendered.startswith("intro\n<!-- benchmarks:python -->\n| Library | Flat (all) |"))
        self.assertIn("| **Scah** | **2.00 ms** |", rendered)
        self.assertIn("| Other | 5.00 ms (2.50×) |", rendered)
        self.assertTrue(rendered.endswith("<!-- /benchmarks:python -->\noutro\n"))
        self.assertEqual(report.render_readme(rendered), rendered)


if __name__ == "__main__":
    unittest.main()
