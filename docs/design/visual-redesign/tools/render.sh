#!/bin/bash
# Renders one mockup board to a PNG, for example:  tools/render.sh Main dark [WIDTH HEIGHT]
# Needs the kit built first:  python3 tools/build_kit.py <phosphor assets/regular> /tmp/kc-kit
# and Playwright (NODE_PATH must reach it).
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
KIT="${KC_KIT:-/tmp/kc-kit}"
OUT="${KC_OUT:-/tmp/kc-render}"
mkdir -p "$OUT"
sed -e "s#@@KC@@#$KIT/kc.css#" -e "s#{{theme}}#$2#g" "$HERE/../mockups/$1.dc.html" > "$OUT/$1-$2.html"
node "$HERE/shot.js" "$OUT/$1-$2.html" "$OUT/$1-$2.png" "${3:-1366}" "${4:-768}"
