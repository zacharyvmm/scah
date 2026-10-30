"""Compare base and candidate binding timings written by the gate rounds.

Each round writes `<language>-<build>-<round>.json`, mapping a workload to its
fastest time in nanoseconds. A workload fails when the median of its
per-round candidate/base ratios exceeds the limit.
"""

import argparse
import json
import pathlib
import statistics
import sys


def load(directory, language, build, round_number):
    path = directory / f"{language}-{build}-{round_number}.json"
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=pathlib.Path)
    parser.add_argument("--rounds", type=int, required=True)
    parser.add_argument("--limit", type=float, required=True)
    parser.add_argument("--languages", nargs="+", required=True)
    args = parser.parse_args()

    failed = False
    for language in args.languages:
        rounds = range(1, args.rounds + 1)
        base = [load(args.directory, language, "base", r) for r in rounds]
        candidate = [load(args.directory, language, "candidate", r) for r in rounds]

        print(f"\n{language} (limit {args.limit:.2f}x)")
        print(f"{'workload':<20} {'base':>12} {'candidate':>12} {'ratio':>7}")
        for workload in base[0]:
            ratios = [c[workload] / b[workload] for b, c in zip(base, candidate)]
            ratio = statistics.median(ratios)
            base_ms = statistics.median(b[workload] for b in base) / 1e6
            candidate_ms = statistics.median(c[workload] for c in candidate) / 1e6
            verdict = "FAIL" if ratio > args.limit else "ok"
            failed |= ratio > args.limit
            print(
                f"{workload:<20} {base_ms:>10.3f}ms {candidate_ms:>10.3f}ms "
                f"{ratio:>6.3f}x {verdict}"
            )

    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
