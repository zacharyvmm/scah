#!/usr/bin/env bash
# Build the PGO Python wheel inside a manylinux container, so it runs on the
# same Linux distributions as a plain `maturin-action` wheel.
#
# Usage: crates/bindings/pgo/manylinux.sh IMAGE WHEEL_DIR
#   IMAGE      e.g. quay.io/pypa/manylinux2014_x86_64
#   WHEEL_DIR  relative to the repository root, e.g. crates/bindings/scah-python/dist
set -euo pipefail

image="${1:?usage: manylinux.sh IMAGE WHEEL_DIR}"
wheels="${2:?usage: manylinux.sh IMAGE WHEEL_DIR}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"

# The container runs as root; hand its output back to the checkout's owner.
docker run --rm -v "$repo_root:/io" -w /io -e WHEELS="$wheels" "$image" bash -euo pipefail -c '
  curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --component llvm-tools
  source "$HOME/.cargo/env"
  python=/opt/python/cp312-cp312/bin/python
  "$python" -m pip install --quiet maturin
  PYTHON="$python" MATURIN="${python%/python}/maturin" \
    crates/bindings/pgo/build.sh python "$WHEELS" --compatibility manylinux2014
  chown -R "$(stat -c %u:%g /io)" /io/target "/io/$WHEELS"
'
