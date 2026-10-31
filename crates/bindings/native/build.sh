#!/usr/bin/env bash
# Build scah's shared libraries for one platform, as the C, C#, and Java
# packages ship them.
#
# Usage: crates/bindings/native/build.sh RID OUT_DIR
#
# RID names the platform with a .NET runtime identifier, which every package
# uses: linux-x64, linux-arm64, linux-musl-x64, linux-musl-arm64, osx-x64,
# osx-arm64, win-x64, or win-arm64. The libraries land in OUT_DIR/RID.
#
# SCAH_NATIVE_CRATES lists the crates to build (default: scah-c). Each must
# build a cdylib named after the crate, with `-` replaced by `_`.
#
# glibc builds run in a manylinux2014 container, so the libraries load on
# glibc 2.17 and newer; musl builds run in Alpine. Both need Docker on a host
# of the same architecture. Other platforms build on the host.
set -euo pipefail

rid="${1:?usage: build.sh RID OUT_DIR}"
out="${2:?usage: build.sh RID OUT_DIR}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
read -r -a crates <<< "${SCAH_NATIVE_CRATES:-scah-c}"

image=""
case "$rid" in
  linux-x64) target=x86_64-unknown-linux-gnu image=quay.io/pypa/manylinux2014_x86_64 ;;
  linux-arm64) target=aarch64-unknown-linux-gnu image=quay.io/pypa/manylinux2014_aarch64 ;;
  linux-musl-x64) target=x86_64-unknown-linux-musl image=rust:alpine ;;
  linux-musl-arm64) target=aarch64-unknown-linux-musl image=rust:alpine ;;
  osx-x64) target=x86_64-apple-darwin ;;
  osx-arm64) target=aarch64-apple-darwin ;;
  win-x64) target=x86_64-pc-windows-msvc ;;
  win-arm64) target=aarch64-pc-windows-msvc ;;
  *)
    echo "unknown RID: $rid" >&2
    exit 2
    ;;
esac

case "$rid" in
  linux-*) prefix=lib suffix=.so ;;
  osx-*) prefix=lib suffix=.dylib ;;
  win-*) prefix="" suffix=.dll ;;
esac

# Linux builds rerun this script in their container, with the repository
# mounted at /io. OUT_DIR must then lie inside the repository.
if [[ -n "$image" && -z "${SCAH_NATIVE_IN_CONTAINER:-}" ]]; then
  mkdir -p "$out"
  out_abs="$(cd "$out" && pwd)"
  case "$out_abs" in
    "$repo_root"/*) ;;
    *)
      echo "OUT_DIR must lie inside the repository for Linux builds" >&2
      exit 2
      ;;
  esac
  # The container runs as root; hand its output back to the checkout's owner.
  docker run --rm -v "$repo_root:/io" -w /io \
    -e SCAH_NATIVE_IN_CONTAINER=1 -e SCAH_NATIVE_CRATES="${crates[*]}" \
    -e RID="$rid" -e OUT="/io/${out_abs#"$repo_root"/}" "$image" sh -euc '
      if command -v apk > /dev/null; then
        apk add --no-cache bash musl-dev > /dev/null
      else
        curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs \
          | sh -s -- -y --profile minimal > /dev/null
        . "$HOME/.cargo/env"
      fi
      owner="$(stat -c %u:%g /io)"
      status=0
      bash crates/bindings/native/build.sh "$RID" "$OUT" || status=$?
      chown -R "$owner" /io/target/native "$OUT"
      exit "$status"
    '
  exit
fi

# Keep each platform's build apart from the host's own target directory.
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo_root/target/native/$rid}"
# Cargo on Windows needs a Windows path, not the POSIX one Git Bash uses.
if command -v cygpath > /dev/null; then
  CARGO_TARGET_DIR="$(cygpath -m "$CARGO_TARGET_DIR")"
fi
export CARGO_TARGET_DIR
target_flags="CARGO_TARGET_$(tr 'a-z-' 'A-Z_' <<< "$target")_RUSTFLAGS"
case "$target" in
  # musl targets link the C runtime statically by default, which rules out
  # shared libraries.
  *-musl) export "$target_flags=-C target-feature=-crt-static" ;;
esac

if command -v rustup > /dev/null; then
  rustup target add "$target" > /dev/null
fi
packages=()
for crate in "${crates[@]}"; do
  packages+=(-p "$crate")
done
cargo build --release --locked --target "$target" "${packages[@]}"

mkdir -p "$out/$rid"
for crate in "${crates[@]}"; do
  file="$prefix${crate//-/_}$suffix"
  cp "$CARGO_TARGET_DIR/$target/release/$file" "$out/$rid/$file"
  echo "built $out/$rid/$file"
done
