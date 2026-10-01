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
