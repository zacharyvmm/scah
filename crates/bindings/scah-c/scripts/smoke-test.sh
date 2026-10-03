#!/usr/bin/env bash
# Build libscah_c and run the C and C++ smoke tests against it.
#
# Usage: crates/bindings/scah-c/scripts/smoke-test.sh [--debug]
#
# Builds in release mode unless --debug is given. Honors CC and CXX.
set -euo pipefail

crate_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$crate_dir"

profile=release
profile_dir=release
if [[ "${1:-}" == "--debug" ]]; then
  profile=dev
  profile_dir=debug
fi

# Build both library kinds and ask rustc which system libraries a static link
# needs. Cargo replays the note when the build is already fresh.
if ! build_log="$(cargo rustc -p scah-c --lib --profile "$profile" \
  --crate-type staticlib --crate-type cdylib \
  -- --print native-static-libs 2>&1)"; then
  echo "$build_log" >&2
  exit 1
fi
native_libs="$(sed -n 's/^note: native-static-libs: //p' <<<"$build_log" | tail -n 1)"
if [[ -z "$native_libs" ]]; then
  case "$(uname -s)" in
    Darwin) native_libs="-lSystem -lc -lm" ;;
    *) native_libs="-lpthread -ldl -lm" ;;
  esac
fi
# cc always links libSystem on macOS, and ld warns when it is named twice.
native_libs="$(sed 's/-lSystem//g' <<<"$native_libs")"
read -r -a native_libs <<<"$native_libs"
echo "native libraries: ${native_libs[*]}"

target_dir="$(cargo metadata --format-version 1 --no-deps |
  sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
lib_dir="$target_dir/$profile_dir"
out_dir="$(mktemp -d)"
trap 'rm -rf "$out_dir"' EXIT

flags=(-Wall -Wextra -Werror -pedantic -I"$crate_dir/include")
# Name the archive directly so the linker cannot pick the shared library.
static_link=("$lib_dir/libscah_c.a" "${native_libs[@]}")

echo "C11, static"
"${CC:-cc}" -std=c11 "${flags[@]}" tests/smoke.c "${static_link[@]}" -o "$out_dir/smoke_c"
"$out_dir/smoke_c"

echo "C++17, static"
"${CXX:-c++}" -std=c++17 "${flags[@]}" tests/smoke.cpp "${static_link[@]}" -o "$out_dir/smoke_cpp"
"$out_dir/smoke_cpp"

echo "C11, shared"
"${CC:-cc}" -std=c11 "${flags[@]}" tests/smoke.c \
  -L"$lib_dir" -lscah_c -Wl,-rpath,"$lib_dir" -o "$out_dir/smoke_c_shared"
"$out_dir/smoke_c_shared"
