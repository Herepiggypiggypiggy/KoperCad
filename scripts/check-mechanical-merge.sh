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
  if git diff "$auto" "$c" -- CLAUDE.md | grep -E '^[+-]' | grep -vE '^(\+\+\+|---) ' |
     grep -vqE '^[+-](\| `novo-[a-z-]+` \||<<<<<<< |=======$|>>>>>>> )'; then
    echo "DIFFERENT: $c edits CLAUDE.md beyond crate-list lines" >&2; exit 1
  fi
  # In Cargo.toml the resolution may only drop conflict markers and exact
  # duplicates (a dropped line must still be in the result), and may only add
  # crate-list or dependency lines taken verbatim from a parent. Nothing of
  # either side is lost or reverted, and no crate is listed twice.
  # (Rule by main reviewer A, review of #61.)
  parents_toml=$( { git show "$p1:Cargo.toml"; git show "$p2:Cargo.toml"; } 2>/dev/null || true )
  result_toml=$(git show "$c:Cargo.toml" 2>/dev/null || true)
  bad=$(git diff "$auto" "$c" -- Cargo.toml | grep -E '^[+-]' | grep -vE '^(\+\+\+|---) ' |
        grep -vE '^-(<<<<<<< |=======$|>>>>>>> )' |
        while IFS= read -r line; do
          body=${line:1}
          case $line in
            -*) grep -qxF -- "$body" <<<"$result_toml" || printf 'dropped: %s\n' "$body" ;;
            +*) if ! grep -qE '^([A-Za-z0-9_-]+ = |# )' <<<"$body" || ! grep -qxF -- "$body" <<<"$parents_toml"; then
                  printf 'added: %s\n' "$body"; fi ;;
          esac
        done || true)
  dup=$(sed -n '/^\[workspace.dependencies\]/,/^\[/p' <<<"$result_toml" | grep -oE '^[A-Za-z0-9_-]+ = ' | sort | uniq -d || true)
  [ -n "$dup" ] && bad="$bad${bad:+$'\n'}listed twice: $dup"
  if [ -n "$bad" ]; then
    echo "DIFFERENT: $c resolves Cargo.toml beyond keeping both sides:" >&2
    printf '%s\n' "$bad" >&2; exit 1
  fi
  prev=$c
done
echo "OK: $head is $reviewed plus mechanical main merges"
