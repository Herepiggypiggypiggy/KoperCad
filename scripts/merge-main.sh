#!/usr/bin/env bash
# Merge origin/main into the checked-out pull request branch and resolve the
# conflicts that are purely mechanical:
#   - CLAUDE.md and Cargo.toml: both sides are kept (union), then duplicate
#     crate rows and duplicate `novo-* = { path = ... }` lines are dropped.
#   - Cargo.lock: main's version, then cargo re-resolves it.
# Any other conflict aborts the merge and exits 1, so a person (or thread)
# resolves it by hand.
#
# Usage: scripts/merge-main.sh            (from the root of a PR worktree)
set -euo pipefail

main=${MAIN_REF:-origin/main}
[ "$main" = origin/main ] && git fetch -q origin main
if git merge -q --no-edit "$main" >/dev/null 2>&1; then
  echo "merged $main cleanly"
  exit 0
fi

mapfile -t unmerged < <(git diff --name-only --diff-filter=U)
for f in "${unmerged[@]}"; do
  case "$f" in
    CLAUDE.md|Cargo.toml|Cargo.lock) ;;
    *)
      echo "conflict in $f is not mechanical; aborting" >&2
      git merge --abort
      exit 1
      ;;
  esac
done

for f in "${unmerged[@]}"; do
  case "$f" in
    Cargo.lock)
      git checkout -q "$main" -- Cargo.lock
      ;;
    *)
      tmp=$(mktemp -d)
      git show ":1:$f" > "$tmp/base" 2>/dev/null || : > "$tmp/base"
      git show ":2:$f" > "$tmp/ours"
      git show ":3:$f" > "$tmp/theirs"
      git merge-file -p --union "$tmp/ours" "$tmp/base" "$tmp/theirs" > "$tmp/merged" || true
      # Drop exact duplicates of crate rows and workspace path entries that
      # the union can produce; keep the first occurrence.
      awk '
        /^\| `novo-[a-z-]+` \|/ || /^novo-[a-z-]+ = \{ path = / {
          if (seen[$0]++) next
        }
        { print }
      ' "$tmp/merged" > "$f"
      rm -rf "$tmp"
      ;;
  esac
  git add "$f"
done

# Let cargo bring Cargo.lock in line with the merged manifests.
cargo metadata --format-version 1 >/dev/null
git add Cargo.lock
git -c core.editor=true commit -q --no-edit
echo "merged $main; resolved mechanically: ${unmerged[*]}"
