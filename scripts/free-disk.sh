#!/usr/bin/env bash
# Free disk in a cloud session before the full workspace test run.
# Cloud disks have a fixed allowance; debug targets of this workspace reach many GB.
# Usage: scripts/free-disk.sh [--deep]   (--deep also removes the whole target dir)
set -euo pipefail
cd "$(dirname "$0")/.."

before=$(df -Pk . | awk 'NR==2{print $4}')

if [ "${1:-}" = "--deep" ]; then
  rm -rf target
else
  # Incremental caches and examples are the bulk and are cheap to rebuild.
  rm -rf target/debug/incremental target/debug/examples target/*/examples 2>/dev/null || true
  # Test binaries older than a day are from earlier builds of this branch.
  find target/debug/deps -type f -executable -mtime +1 -delete 2>/dev/null || true
fi
# Old per-session scratch targets next to the repo.
rm -rf ../kopercad-target-* 2>/dev/null || true
command -v cargo >/dev/null && rm -rf "${CARGO_HOME:-$HOME/.cargo}/registry/src" 2>/dev/null || true

after=$(df -Pk . | awk 'NR==2{print $4}')
echo "freed $(( (after - before) / 1024 )) MB, $(( after / 1024 )) MB available"
