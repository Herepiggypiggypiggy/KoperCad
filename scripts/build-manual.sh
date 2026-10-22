#!/usr/bin/env bash
# Builds the user manual with fresh screenshots (docs/manual/README.md).
#
#   scripts/build-manual.sh [OUT_DIR]       default: target/manual
#
# Runs the QA scenario that takes the manual's pictures at 1920x1080, copies them
# to docs/manual/images/<name>.png (these are what the program bundles), then writes
# the site with kopercad-cli. Needs the same Linux packages as scripts/qa.sh.
set -euo pipefail
cd "$(dirname "$0")/.."
out=${1:-target/manual}
qa=target/manual-qa
rm -rf "$qa"
scripts/qa.sh --only manual-help --size 1920x1080 --out "$qa" >&2
mkdir -p docs/manual/images
for shot in "$qa"/shots/80-manual-help-1920x1080-*.png; do
  [ -e "$shot" ] || continue
  name=${shot##*-1920x1080-}
  name=${name#[0-9][0-9]-}
  case $name in finding-*) continue ;; esac
  cp "$shot" "docs/manual/images/${name}"
done
bin=${KOPERCAD_BIN_DIR:-target/debug}
"$bin/kopercad-cli" manual "$out" --images docs/manual/images
