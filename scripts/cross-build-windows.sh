#!/usr/bin/env bash
# Builds and checks the Windows version from Linux, with no GitHub Actions and
# no Windows machine. Used by Claude sessions and by anyone with a Linux box.
#
#   scripts/cross-build-windows.sh check     # clippy -D warnings for Windows
#   scripts/cross-build-windows.sh zip DIR   # release build into DIR/novoelectric-<sha>-windows.zip
#
# Uses the GNU target with MinGW-w64 (apt: gcc-mingw-w64-x86-64). The result
# links only system DLLs and needs no install or admin rights on Windows, but
# it has not been run on Windows by this script: only compiled and linked.
set -euo pipefail
cd "$(dirname "$0")/.."
target=x86_64-pc-windows-gnu
command -v x86_64-w64-mingw32-gcc > /dev/null || { echo "install gcc-mingw-w64-x86-64 first" >&2; exit 1; }
rustup target add "$target" > /dev/null

case "${1:-}" in
  check)
    cargo clippy --workspace --all-targets --target "$target" -- -D warnings
    ;;
  zip)
    out=${2:?usage: cross-build-windows.sh zip DIR}
    sha=$(git rev-parse --short HEAD)
    CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
      cargo build --release -p novo-app --target "$target"
    stage=$(mktemp -d)
    trap 'rm -rf "$stage"' EXIT
    x86_64-w64-mingw32-strip -o "$stage/novoelectric.exe" "${CARGO_TARGET_DIR:-target}/$target/release/novoelectric.exe"
    printf 'KoperCAD %s, cross-built on Linux %s.\nNo install needed: unzip anywhere in your user folder and run novoelectric.exe.\nCompiled and linked only; not run on Windows by the build.\n' \
      "$sha" "$(date -u +%Y-%m-%dT%H:%MZ)" > "$stage/BUILD_INFO.txt"
    mkdir -p "$out"
    out=$(cd "$out" && pwd)
    (cd "$stage" && python3 -c "
import zipfile,sys
z=zipfile.ZipFile(sys.argv[1],'w',zipfile.ZIP_DEFLATED)
[z.write(f) for f in ('novoelectric.exe','BUILD_INFO.txt')]
z.close()" "$out/novoelectric-$sha-windows.zip")
    echo "$out/novoelectric-$sha-windows.zip"
    ;;
  *)
    echo "usage: $0 check | zip DIR" >&2
    exit 2
    ;;
esac
