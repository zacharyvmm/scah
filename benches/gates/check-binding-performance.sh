#!/usr/bin/env bash
# Compare the Python and Node bindings of this checkout against a base revision.
#
# Both revisions are built in release mode, then each binding runs the shared
# workloads in benches/gates/bindings in alternating rounds. The gate fails when
# any workload's median candidate/base ratio exceeds SCAH_BINDING_GATE_LIMIT.
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
base_ref="${1:-origin/main}"
gate_root="$(mktemp -d -t scah-binding-performance.XXXXXX)"
base_tree="$gate_root/base"
results="$gate_root/results"
# Use an even number of alternating rounds so each build runs first equally
# often, which keeps thermal or frequency drift from favoring either build.
rounds="${SCAH_PERF_GATE_ROUNDS:-4}"
limit="${SCAH_BINDING_GATE_LIMIT:-1.05}"
read -r -a languages <<< "${SCAH_BINDING_GATE_LANGUAGES:-python node}"
python="${PYTHON:-python3}"

cleanup() {
  if git -C "$repo_root" worktree list --porcelain | grep -Fqx "worktree $base_tree"; then
    git -C "$repo_root" worktree remove --force "$base_tree"
  fi
  rm -rf "$gate_root"
}
trap cleanup EXIT

git -C "$repo_root" worktree add --detach "$base_tree" "$base_ref"
mkdir -p "$results"

# Unpack each wheel instead of installing it so both builds can share one
# interpreter, selected per run through PYTHONPATH.
build_python() {
  local source_root="$1"
  local destination="$2"
  CARGO_TARGET_DIR="$gate_root/$destination-target" uvx maturin build --release \
    --manifest-path "$source_root/crates/bindings/scah-python/Cargo.toml" \
    --interpreter "$python" --out "$gate_root/$destination-wheel"
  "$python" -m zipfile -e "$gate_root/$destination-wheel"/*.whl "$gate_root/$destination-python"
}

build_node() {
  local source_root="$1"
  local destination="$2"
  (
    cd "$source_root/crates/bindings/scah-node"
    bun install --frozen-lockfile
    CARGO_TARGET_DIR="$gate_root/$destination-target" bun run build
  )
}

run_round() {
  local language="$1"
  local build="$2"
  local round_number="$3"
  local source_root="$base_tree"
  [[ "$build" == candidate ]] && source_root="$repo_root"
  local output="$results/$language-$build-$round_number.json"
  case "$language" in
    python)
      PYTHONPATH="$gate_root/$build-python" "$python" \
        "$repo_root/benches/gates/bindings/bench.py" > "$output"
      ;;
    node)
      bun "$repo_root/benches/gates/bindings/bench.ts" \
        "$source_root/crates/bindings/scah-node/index.js" > "$output"
      ;;
  esac
}

for language in "${languages[@]}"; do
  "build_$language" "$base_tree" base
  "build_$language" "$repo_root" candidate
done

for ((round = 1; round <= rounds; round++)); do
  for language in "${languages[@]}"; do
    if ((round % 2 == 0)); then
      run_round "$language" candidate "$round"
      run_round "$language" base "$round"
    else
      run_round "$language" base "$round"
      run_round "$language" candidate "$round"
    fi
  done
done

"$python" "$repo_root/benches/gates/bindings/compare.py" "$results" \
  --rounds "$rounds" --limit "$limit" --languages "${languages[@]}"
