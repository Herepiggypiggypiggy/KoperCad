#!/usr/bin/env bash
# Prove that a pull request's new head is its reviewed head plus mechanical
# merges of main only. Every commit in <reviewed>..<head> must be a two-parent
# merge whose first parent is the previous head and whose second parent is on
# main, with a tree equal to git's automatic merge of those parents, except
# for Cargo.lock and crate-list lines / resolved conflict markers in CLAUDE.md
# and Cargo.toml.
#
# When this passes, the reviewer's READY on <reviewed> carries over to <head>;
# the coordinator pastes this script's output into the merge comment. A
# lock-only difference passes, so `cargo test --workspace` is still re-run on
# the new head before merging. Needs git >= 2.38 (merge-tree --write-tree).
# (Design and first version by main reviewer A, review of #51.)
#
# Usage: scripts/check-mechanical-merge.sh <reviewed-sha> <new-head-sha>
set -euo pipefail
reviewed=$(git rev-parse "$1^{commit}"); head=$(git rev-parse "$2^{commit}")
git merge-base --is-ancestor "$reviewed" "$head" || { echo "DIFFERENT: $reviewed is not an ancestor of $head" >&2; exit 1; }
prev=$reviewed
for c in $(git rev-list --first-parent --reverse "$reviewed..$head"); do
  read -r p1 p2 extra <<<"$(git rev-list --parents -n1 "$c" | cut -d' ' -f2-)"
  [ "$p1" = "$prev" ] && [ -n "${p2:-}" ] && [ -z "${extra:-}" ] || { echo "DIFFERENT: $c is not a two-parent merge on top of $prev" >&2; exit 1; }
  git merge-base --is-ancestor "$p2" origin/main || { echo "DIFFERENT: $c merges $p2, which is not on main" >&2; exit 1; }
  auto=$(git merge-tree --write-tree "$p1" "$p2" | head -n1 || true)
  if ! git diff --quiet "$auto" "$c" -- . ':(exclude)Cargo.lock' ':(exclude)CLAUDE.md' ':(exclude)Cargo.toml'; then
    echo "DIFFERENT: $c changes code beyond git's automatic merge:" >&2
    git diff --stat "$auto" "$c" -- . ':(exclude)Cargo.lock' ':(exclude)CLAUDE.md' ':(exclude)Cargo.toml' >&2; exit 1
  fi
  # The mechanical files may differ from git's merge only in crate-list lines and resolved conflict markers.
  if git diff "$auto" "$c" -- CLAUDE.md Cargo.toml | grep -E '^[+-]' | grep -vE '^(\+\+\+|---) ' |
     grep -vqE '^[+-](\| `novo-[a-z-]+` \||novo-[a-z-]+ = \{ path = |<<<<<<< |=======$|>>>>>>> )'; then
    echo "DIFFERENT: $c edits CLAUDE.md/Cargo.toml beyond crate-list lines" >&2; exit 1
  fi
  prev=$c
done
echo "OK: $head is $reviewed plus mechanical main merges"
