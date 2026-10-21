#!/usr/bin/env bash
# One entry point for the test and screenshot suite (docs/testing/qa-suite.md).
# Builds the app and kopercad-cli when missing, then runs `kopercad-cli qa`.
#
#   scripts/qa.sh                     run the quick scenarios at 1366x768 and 1920x1080
#   scripts/qa.sh --slow              also the sweeps (every command, every lesson)
#   scripts/qa.sh --only cabinet      scenarios whose name or title contains "cabinet"
#   scripts/qa.sh --size 1366x768 --out /mnt/project-files/qa/my-run
#   scripts/qa.sh list                what scenarios exist
set -euo pipefail
cd "$(dirname "$0")/.."
bin=${KOPERCAD_BIN_DIR:-target/debug}
if [ ! -x "$bin/kopercad" ] || [ ! -x "$bin/kopercad-cli" ]; then
  cargo build -p novo-app -p novo-cli >&2
fi
case ${1:-run} in
  list) shift; exec "$bin/kopercad-cli" qa list "$@" ;;
  report) shift; exec "$bin/kopercad-cli" qa report "$@" ;;
  run) [ $# -gt 0 ] && shift; exec "$bin/kopercad-cli" qa run "$@" ;;
  *) exec "$bin/kopercad-cli" qa run "$@" ;;
esac
