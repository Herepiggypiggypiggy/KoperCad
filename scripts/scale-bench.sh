#!/usr/bin/env bash
# Scale benchmark: builds a big synthetic project (hundreds of pages, thousands
# of wires) and times open/save, redraw, search, numbering, checks, reports,
# Autopilot, PDF, 3D and heat. Release build; results print as a Markdown table.
#   scripts/scale-bench.sh                      # medium (200 pages)
#   scripts/scale-bench.sh --size large         # 600 pages
#   scripts/scale-bench.sh --size small --budget
#   scripts/scale-bench.sh --only heat,report   # steps whose name contains a word
# The heat steps take minutes on a 60-device cabinet; skip them with
#   --only generate,save,open,search,page,edit,number,design,report,autopilot,pdf,3d,route
set -euo pipefail
cd "$(dirname "$0")/.."
exec cargo run --release -p novo-tests --example scale_bench -- "$@"
