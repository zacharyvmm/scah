#!/usr/bin/env bash
# Build the ordinary-gate bench with profile-guided optimization and list
# where LLVM placed scah's functions: in `.text.hot` or `.text.unlikely`
# input sections, or neither.
#
# This is PGO's verdict on which code is hot. Compare it with the
# `#[cold]`/`#[inline(never)]` layout hints (build with
# `RUSTFLAGS="--cfg scah_no_layout_hints"` to train without them), or use it
# to decide where hints belong instead of guessing.
#
# Usage: benches/layout/pgo-sections.sh [OUTPUT_DIR]
#
# Writes OUTPUT_DIR/sections.txt (one "hot|unlikely|plain function" line per
# function) and OUTPUT_DIR/speed_bench_ordinary_gate (the PGO build). Needs
# x86-64, lld, and the `llvm-tools` rustup component. Extra rustc flags can
# come from RUSTFLAGS, and apply to both the training and the final build.
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
out="$(realpath -m "${1:-$repo_root/target/pgo-sections}")"
mkdir -p "$out"
work="$(mktemp -d -t scah-pgo.XXXXXX)"
trap 'rm -rf "$work"' EXIT

sysroot="$(rustc --print sysroot)"
host="$(rustc -vV | sed -n 's/^host: //p')"
profdata="$sysroot/lib/rustlib/$host/bin/llvm-profdata"
if [[ ! -x "$profdata" ]]; then
  echo "llvm-profdata not found; install it with: rustup component add llvm-tools"
  exit 1
fi

# bench TARGET_DIR FLAGS...: build the gate bench and print its path.
bench() {
  local target="$1"
  shift
  RUSTFLAGS="${RUSTFLAGS:-} $*" CARGO_TARGET_DIR="$target" cargo bench -q \
    --manifest-path "$repo_root/Cargo.toml" -p scah-benches \
    --bench speed_bench_ordinary_gate --no-run
  ls -t "$target"/release/deps/speed_bench_ordinary_gate-* | grep -v '\.d$' | head -n 1
}

instrumented=$(bench "$work/generate" "-Cprofile-generate=$work/raw")
"$instrumented" --bench ordinary_parser_gate --warm-up-time 0.5 --measurement-time 1 \
  --noplot > /dev/null 2>&1
"$profdata" merge -o "$work/merged.profdata" "$work/raw"

optimized=$(bench "$work/use" "-Cprofile-use=$work/merged.profdata" \
  "-Clink-arg=-fuse-ld=lld" "-Clink-arg=-Wl,-Map=$work/link.map")
cp "$optimized" "$out/speed_bench_ordinary_gate"

# The linker map names each function's input section: .text.hot.<symbol>,
# .text.unlikely.<symbol>, or .text.<symbol>.
grep -oE '\(\.text(\.hot|\.unlikely)?\._ZN[^)]*4scah[^)]*\)' "$work/link.map" \
  | sed -E 's/^\(\.text\.(hot|unlikely)\.(.*)\)$/\1 \2/; s/^\(\.text\.(.*)\)$/plain \1/' \
  | c++filt | sed -E 's/::h[0-9a-f]{16}$//' | sort -u > "$out/sections.txt"

cut -d' ' -f1 "$out/sections.txt" | sort | uniq -c
echo "Functions by section: $out/sections.txt"
