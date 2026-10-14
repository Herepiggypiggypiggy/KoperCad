#!/usr/bin/env bash
# Prove that a pull request's new head differs from its reviewed head only by
# merging main: the PR's own change against main must be identical, apart
# from Cargo.lock and the crate-list lines in CLAUDE.md / Cargo.toml.
#
# When this passes, the reviewer's READY on <reviewed> carries over to <head>
# and the coordinator may merge without asking for a re-review (MAIN-HANDOFF.md).
#
# Usage: scripts/check-mechanical-merge.sh <reviewed-sha> <new-head-sha>
set -euo pipefail

reviewed=$1
head=$2
git fetch -q origin main

# The PR's change as reviewed (against the main it was based on) and as it
# stands now (against current main).
before_base=$(git merge-base origin/main "$reviewed")
after_base=$(git merge-base origin/main "$head")

normalize() {
  # Keep only the PR's own added and removed lines (context shifts when main
  # moves), minus the crate-list lines a main merge may reshuffle.
  grep -v -E '^(index |@@ | )' |
    grep -v -E '^[ +-]\| `novo-[a-z-]+` \|' |
    grep -v -E '^[ +-]novo-[a-z-]+ = \{ path = '
}

pathspec=(-- . ':(exclude)Cargo.lock')
git diff "$before_base" "$reviewed" "${pathspec[@]}" | normalize > /tmp/mech-before.$$
git diff "$after_base" "$head" "${pathspec[@]}" | normalize > /tmp/mech-after.$$

if diff -q /tmp/mech-before.$$ /tmp/mech-after.$$ >/dev/null; then
  echo "OK: $head carries exactly the change reviewed at $reviewed"
  rm -f /tmp/mech-before.$$ /tmp/mech-after.$$
  exit 0
fi
echo "DIFFERENT: the PR's own change moved between $reviewed and $head:" >&2
diff /tmp/mech-before.$$ /tmp/mech-after.$$ | head -60 >&2
rm -f /tmp/mech-before.$$ /tmp/mech-after.$$
exit 1
