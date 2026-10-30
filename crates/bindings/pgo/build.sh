#!/usr/bin/env bash
# Build the Python or Node binding with profile-guided optimization (PGO).
#
#   1. Build the binding instrumented to count which code runs.
#   2. Run train.py or train.mjs through it.
#   3. Merge the counts into one profile.
#   4. Rebuild the binding optimized with that profile.
#
# Usage:
#   crates/bindings/pgo/build.sh python WHEEL_DIR [MATURIN_ARGS...]
#   crates/bindings/pgo/build.sh node [NAPI_ARGS...]
#
# The Python wheel lands in WHEEL_DIR. The Node addon lands next to index.js,
# as with `bun run build`. Extra arguments go to both builds. Needs the
# `llvm-tools` rustup component; MATURIN, PYTHON, BUN, and NODE select the
# tools.
set -euo pipefail

binding="${1:?usage: build.sh python WHEEL_DIR [ARGS...] | build.sh node [ARGS...]}"
shift
pgo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bindings_dir="$(dirname "$pgo_dir")"
work="$(dirname "$(dirname "$bindings_dir")")/target/pgo/$binding"
profiles="$work/profiles"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$work/target}"
read -r -a maturin <<< "${MATURIN:-maturin}"
python="${PYTHON:-python3}"
bun="${BUN:-bun}"
node="${NODE:-node}"

# rustc on Windows needs Windows paths, not the POSIX ones Git Bash uses.
native_path() {
  if command -v cygpath > /dev/null; then cygpath -m "$1"; else printf '%s\n' "$1"; fi
}

find_profdata() {
  local sysroot host
  sysroot="$(rustc --print sysroot)"
  command -v cygpath > /dev/null && sysroot="$(cygpath -u "$sysroot")"
  host="$(rustc -vV | sed -n 's/^host: //p')"
  for tool in "$sysroot/lib/rustlib/$host/bin/llvm-profdata"{,.exe}; do
    if [[ -x "$tool" ]]; then
      printf '%s\n' "$tool"
      return
    fi
  done
  echo "llvm-profdata not found; install it with: rustup component add llvm-tools" >&2
  exit 1
}

# build_python RUSTFLAG WHEEL_DIR MATURIN_ARGS...
build_python() {
  local flag="$1" wheels="$2"
  shift 2
  rm -rf "$wheels"
  (
    cd "$bindings_dir/scah-python"
    RUSTFLAGS="${RUSTFLAGS:-} $flag" "${maturin[@]}" build --release --out "$wheels" "$@"
  )
}

train_python() {
  rm -rf "$work/site"
  "$python" -m zipfile -e "$work"/instrumented/*.whl "$work/site"
  PYTHONPATH="$(native_path "$work/site")" "$python" "$pgo_dir/train.py"
}

# build_node RUSTFLAG NAPI_ARGS...
build_node() {
  local flag="$1"
  shift
  (
    cd "$bindings_dir/scah-node"
    "$bun" install --frozen-lockfile
    RUSTFLAGS="${RUSTFLAGS:-} $flag" "$bun" run build "$@"
  )
}

train_node() {
  "$node" "$pgo_dir/train.mjs" "$bindings_dir/scah-node/index.js"
}

profdata="$(find_profdata)"
mkdir -p "$work"
case "$binding" in
  python)
    mkdir -p "${1:?build.sh python needs a wheel directory}"
    wheels="$(cd "$1" && pwd)"
    shift
    build_python "-Cprofile-generate=$(native_path "$profiles")" "$work/instrumented" "$@"
    ;;
  node) build_node "-Cprofile-generate=$(native_path "$profiles")" "$@" ;;
  *) echo "unknown binding: $binding (expected python or node)" >&2; exit 1 ;;
esac

# Build scripts and proc macros are instrumented too when building for the
# host; drop what they recorded during the build.
rm -rf "$profiles"
"train_$binding"
"$profdata" merge -o "$work/merged.profdata" "$profiles"

# Cargo rebuilds when RUSTFLAGS change but not when the profile they name
# does, so name the profile after its contents.
profile="$work/merged-$(cksum < "$work/merged.profdata" | cut -d' ' -f1).profdata"
mv "$work/merged.profdata" "$profile"
use="-Cprofile-use=$(native_path "$profile")"
case "$binding" in
  python) build_python "$use" "$wheels" "$@" ;;
  node) build_node "$use" "$@" ;;
esac
