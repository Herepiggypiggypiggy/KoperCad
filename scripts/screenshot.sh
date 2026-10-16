#!/usr/bin/env bash
# Headless screenshot of a GUI program, for before and after pictures in pull
# requests. Needs Xvfb, openbox, xdotool, ImageMagick (import) and
# mesa-vulkan-drivers; Linux only (apt install xvfb openbox xdotool
# imagemagick mesa-vulkan-drivers libxkbcommon-x11-0).
#
#   scripts/screenshot.sh out.png [-d SECONDS] [-k "ctrl+3 ctrl+k"] [-c "x,y"] -- command args...
#
# -d  seconds to wait before each capture step (default 8)
# -k  xdotool key sequence sent after the wait, then captured again as out-2.png ...
# -c  click at x,y (screen pixels, window origin is about 80,38) before the keys
#
# Example: scripts/screenshot.sh /tmp/home.png -c 613,383 -- target/debug/kopercad
set -euo pipefail
out=$1; shift
delay=8; keys=""; click=""
while [ $# -gt 0 ]; do
  case $1 in
    -d) delay=$2; shift 2 ;;
    -k) keys=$2; shift 2 ;;
    -c) click=$2; shift 2 ;;
    --) shift; break ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
done
[ $# -gt 0 ] || { echo "no command given" >&2; exit 2; }
disp=:${SCREENSHOT_DISPLAY:-98}
export DISPLAY=$disp
export HOME=${SCREENSHOT_HOME:-$(mktemp -d)}   # empty settings: first-run look
Xvfb "$disp" -screen 0 ${SCREENSHOT_RES:-1440x900}x24 >/dev/null 2>&1 & xvfb=$!
sleep 1
openbox >/dev/null 2>&1 & wm=$!
"$@" >/dev/null 2>&1 & app=$!
cleanup() { kill "$app" "$wm" "$xvfb" 2>/dev/null || true; }
trap cleanup EXIT
sleep "$delay"
if [ -n "$click" ]; then
  xdotool mousemove "${click%,*}" "${click#*,}" click 1; sleep 3
fi
import -window root "$out"
n=2
for k in $keys; do
  xdotool key "$k"; sleep 2
  import -window root "${out%.png}-$n.png"; n=$((n+1))
done
