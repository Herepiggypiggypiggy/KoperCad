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
# Linux without a screen needs a virtual one: name what is missing before the run fails.
if [ "$(uname -s)" = Linux ] && [ -z "${DISPLAY:-}" ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  missing=""
  command -v Xvfb >/dev/null || missing="$missing xvfb"
  command -v openbox >/dev/null || missing="$missing openbox"
  ls /usr/share/vulkan/icd.d/lvp_icd.json >/dev/null 2>&1 || missing="$missing mesa-vulkan-drivers libvulkan1"
  [ -n "$(ldconfig -p 2>/dev/null | grep libxkbcommon-x11.so)" ] || missing="$missing libxkbcommon-x11-0"
  if [ -n "$missing" ]; then
    echo "qa.sh: missing on this Linux box:$missing" >&2
    echo "  sudo apt-get update && sudo apt-get install -y$missing   (as root: drop sudo)" >&2
    echo "  no admin rights: ask for these packages in the environment setup script instead" >&2
    exit 3
  fi
fi
# Own build: always bring it up to date (incremental, cheap), so a run never tests an old binary.
# The lessons are embedded at build time and cargo can miss a changed lesson file: force the embed
# to rebuild when any lesson is newer than the program.
if [ -z "${KOPERCAD_BIN_DIR:-}" ]; then
  if [ -x "$bin/kopercad" ] && [ -n "$(find assets/lessons -type f -newer "$bin/kopercad" -print -quit)" ]; then
    touch crates/novo-app/build.rs
  fi
  cargo build -p novo-app -p novo-cli >&2
elif [ ! -x "$bin/kopercad" ] || [ ! -x "$bin/kopercad-cli" ]; then
  echo "qa.sh: no kopercad and kopercad-cli in $bin" >&2
  exit 3
fi
case ${1:-run} in
  list) shift; exec "$bin/kopercad-cli" qa list "$@" ;;
  report) shift; exec "$bin/kopercad-cli" qa report "$@" ;;
  run) [ $# -gt 0 ] && shift; exec "$bin/kopercad-cli" qa run "$@" ;;
  *) exec "$bin/kopercad-cli" qa run "$@" ;;
esac
