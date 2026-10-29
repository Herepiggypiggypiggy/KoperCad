#!/usr/bin/env bash
# Builds the controller add-on: engine.wasm (novo-controller behind host API 1) and the
# package dist/controllers-<version>.kcaddon. Used by CI and by hand; needs the Rust target
# wasm32-unknown-unknown (rustup target add wasm32-unknown-unknown). No admin rights needed.
#
#   scripts/build-controllers-addon.sh            # unsigned package
#   KOPERCAD_SIGN_KEY_ID=koper-1 KOPERCAD_SIGN_SEED_FILE=seed.hex scripts/build-controllers-addon.sh
#
# engine.wasm may not exceed 4 MiB (the program refuses a bigger one); this script fails first.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
engine="$root/addons/controllers/engine"
stage="${CONTROLLERS_STAGE:-$(mktemp -d)}"
dist="${CONTROLLERS_DIST:-$root/dist}"
mkdir -p "$stage" "$dist"

cargo build --release --locked --target wasm32-unknown-unknown --manifest-path "$engine/Cargo.toml" --target-dir "$engine/target"
cp -r "$root/addons/controllers/pack/." "$stage/"
cp "$engine/target/wasm32-unknown-unknown/release/controllers_engine.wasm" "$stage/engine.wasm"

size=$(wc -c < "$stage/engine.wasm")
echo "engine.wasm: $size bytes"
if [ "$size" -gt 4194304 ]; then
  echo "engine.wasm is larger than 4 MiB" >&2
  exit 1
fi

version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$stage/manifest.toml" | head -1)
out="$dist/controllers-$version.kcaddon"
sign=()
if [ -n "${KOPERCAD_SIGN_KEY_ID:-}" ] && [ -n "${KOPERCAD_SIGN_SEED_FILE:-}" ]; then
  sign=(--key-id "$KOPERCAD_SIGN_KEY_ID" --seed-file "$KOPERCAD_SIGN_SEED_FILE")
fi
cargo run -q -p novo-cli --manifest-path "$root/Cargo.toml" -- addons pack "$stage" --output "$out" "${sign[@]+"${sign[@]}"}"
echo "$out"
