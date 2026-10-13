#!/usr/bin/env bash
# Compare this checkout's ordinary-gate parse times with a base revision's
# across several code layouts.
#
# The parse loop's speed depends on where its code lands: on Intel CPUs the
# share of it served from the decoded-µop cache moves by over 20 points
# between builds that differ only in function order. One build per side, as
# the CI gate compares, can show a few percent either way that no source
# change caused. This script links each side with lld's `--shuffle-sections`
# under several seeds (seed 0 keeps the default order), alternates runs per
# seed, and reports the mean change.
#
# Usage: benches/layout/compare.sh [BASE_REF] [SEEDS]
#
# Environment:
#   BASE_RUSTFLAGS, CANDIDATE_RUSTFLAGS  extra rustc flags per side, e.g.
#       CANDIDATE_RUSTFLAGS="--cfg scah_no_layout_hints" with BASE_REF=HEAD
#       to measure the layout hints on one tree
#   ROUNDS  alternating runs per seed and workload (default 3)
#   CPU     core to pin runs to with taskset (default: unpinned)
#
# Needs x86-64, lld, and python3. With access to hardware counters
# (kernel.perf_event_paranoid <= 2) it also reports each build's IPC and,
# on Intel, the share of µops served from the decoded-µop cache.
set -euo pipefail

if [[ "$(uname -m)" != "x86_64" ]]; then
  echo "The layout comparison must run on x86-64."
  exit 1
fi

repo_root="$(git rev-parse --show-toplevel)"
base_ref="${1:-origin/main}"
seeds="${2:-4}"
rounds="${ROUNDS:-3}"
base_label="$(git rev-parse --short "$base_ref")"
root="$(mktemp -d -t scah-layout.XXXXXX)"
base_tree="$root/base"

cleanup() {
  if git -C "$repo_root" worktree list --porcelain | grep -Fqx "worktree $base_tree"; then
    git -C "$repo_root" worktree remove --force "$base_tree"
  fi
  rm -rf "$root"
}
trap cleanup EXIT

git -C "$repo_root" worktree add --detach "$base_tree" "$base_ref" > /dev/null
mkdir -p "$base_tree/benches/ordinary_gate"
cp "$repo_root/benches/ordinary_gate/speed_bench.rs" \
  "$base_tree/benches/ordinary_gate/speed_bench.rs"
if ! grep -Fq 'name = "speed_bench_ordinary_gate"' "$base_tree/benches/Cargo.toml"; then
  printf '\n[[bench]]\nname = "speed_bench_ordinary_gate"\npath = "ordinary_gate/speed_bench.rs"\nharness = false\n' \
    >> "$base_tree/benches/Cargo.toml"
fi

# build TREE SIDE SEED RUSTFLAGS: link the gate bench of TREE into $root/SIDE-SEED.
# The seed reaches only the final link, so dependencies build once per side.
build() {
  local tree="$1" side="$2" seed="$3" flags="$4"
  local link=(-C link-arg=-fuse-ld=lld)
  if [[ "$seed" != 0 ]]; then
    link+=(-C "link-arg=-Wl,--shuffle-sections=*=$seed")
  fi
  RUSTFLAGS="$flags" CARGO_TARGET_DIR="$root/$side-target" cargo rustc -q \
    --manifest-path "$tree/Cargo.toml" -p scah-benches --profile bench \
    --bench speed_bench_ordinary_gate -- "${link[@]}"
  cp "$(ls -t "$root/$side-target"/release/deps/speed_bench_ordinary_gate-* | grep -v '\.d$' | head -n 1)" \
    "$root/$side-$seed"
}

for ((seed = 0; seed < seeds; seed++)); do
  build "$base_tree" base "$seed" "${BASE_RUSTFLAGS:-}"
  build "$repo_root" candidate "$seed" "${CANDIDATE_RUSTFLAGS:-}"
done

pin=()
if [[ -n "${CPU:-}" ]]; then
  pin=(taskset -c "$CPU")
fi

# nanoseconds BIN WORKLOAD: criterion's median time for one run.
nanoseconds() {
  "${pin[@]}" "$1" --bench "ordinary_parser_gate/$2" --warm-up-time 1 --measurement-time 2 \
    --noplot 2>&1 | awk '/time:/ && !seen++ {
      split($0, f, "["); split(f[2], t, " ")
      print (t[4] == "ms") ? t[3] * 1e6 : (t[4] == "µs") ? t[3] * 1e3 : t[3]
    }'
}

counters=0
if perf stat -e cycles,instructions true > /dev/null 2>&1; then
  counters=1
fi
dsb_events=""
if perf stat -e idq.dsb_uops,idq.mite_uops,idq.ms_uops true > /dev/null 2>&1; then
  dsb_events=",idq.dsb_uops,idq.mite_uops,idq.ms_uops"
fi

# frontend BIN WORKLOAD: "IPC x.xx" and, on Intel, "DSB yy%".
frontend() {
  [[ "$counters" == 1 ]] || return 0
  "${pin[@]}" perf stat -x, -e "cycles,instructions$dsb_events" -- "$1" \
    --bench "ordinary_parser_gate/$2" --profile-time 2 2>&1 > /dev/null | awk -F, '
      NF > 3 && $1 ~ /^[0-9]/ {
        # Hybrid CPUs report each PMU separately, as cpu_core/cycles/ and
        # cpu_atom/cycles/; sum them under the plain event name.
        name = $3
        sub(/^cpu_[a-z]+\//, "", name)
        sub(/\/$/, "", name)
        c[name] += $1
      }
      END {
        if (c["cycles"] > 0) printf "IPC %.2f", c["instructions"] / c["cycles"]
        else printf "IPC n/a"
        d = c["idq.dsb_uops"]; m = c["idq.mite_uops"] + c["idq.ms_uops"]
        if (d + m > 0) printf ", DSB %.0f%%", 100 * d / (d + m)
      }'
}

for workload in no_match match; do
  changes=()
  for ((seed = 0; seed < seeds; seed++)); do
    ratios=()
    for ((round = 1; round <= rounds; round++)); do
      if ((round % 2)); then
        base=$(nanoseconds "$root/base-$seed" "$workload")
        candidate=$(nanoseconds "$root/candidate-$seed" "$workload")
      else
        candidate=$(nanoseconds "$root/candidate-$seed" "$workload")
        base=$(nanoseconds "$root/base-$seed" "$workload")
      fi
      ratios+=("$(python3 -c "print($candidate / $base)")")
    done
    change=$(printf '%s\n' "${ratios[@]}" |
      python3 -c "import statistics, sys; print(statistics.median(float(x) for x in sys.stdin) - 1)")
    changes+=("$change")
    detail=""
    if [[ "$counters" == 1 ]]; then
      detail="  base $(frontend "$root/base-$seed" "$workload") | candidate $(frontend "$root/candidate-$seed" "$workload")"
    fi
    printf '%-8s seed %d: %+6.2f%%%s\n' "$workload" "$seed" \
      "$(python3 -c "print(100 * $change)")" "$detail"
  done
  printf '%s\n' "${changes[@]}" | python3 -c "
import sys
values = [float(x) for x in sys.stdin]
print(f'$workload: {100 * sum(values) / len(values):+.2f}% vs $base_label (mean of {len(values)} layouts)')
"
done
