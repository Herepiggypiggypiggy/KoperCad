#!/usr/bin/env bash
# Merge origin/main into the checked-out pull request branch and resolve the
# conflicts that are purely mechanical:
#   - CLAUDE.md and Cargo.toml: only when every conflicting line on both sides
#     is a crate-list line (`| \`novo-x\` | ...` row or `novo-x = { path = ... }`);
#     both sides are kept and exact duplicates dropped. Any other conflict in
#     these files (prose, versions) aborts.
#   - Cargo.lock: main's version, then cargo re-resolves it (`cargo metadata`;
#     needs the network on a cold cache).
# Any other conflict aborts the merge and exits 1, so the owner resolves it.
# Check the result with scripts/check-mechanical-merge.sh <reviewed> HEAD.
#
# Usage: scripts/merge-main.sh            (from the root of a PR worktree)
#        MAIN_REF=<ref> scripts/merge-main.sh   (merge another ref, for tests)
set -euo pipefail

main=${MAIN_REF:-origin/main}
[ "$main" = origin/main ] && git fetch -q origin main
if git merge -q --no-edit "$main" >/dev/null 2>&1; then
  echo "merged $main cleanly"
  exit 0
fi
if ! git rev-parse -q --verify MERGE_HEAD >/dev/null; then
  echo "git refused to start the merge (dirty tree or untracked files in the way); nothing changed" >&2
  exit 1
fi

crate_line='^(\| `novo-[a-z-]+` \||novo-[a-z-]+ = \{ path = )'

mapfile -t unmerged < <(git diff --name-only --diff-filter=U)
for f in "${unmerged[@]}"; do
  case "$f" in
    Cargo.lock) ;;
    CLAUDE.md|Cargo.toml)
      tmp=$(mktemp -d)
      git show ":1:$f" > "$tmp/base" 2>/dev/null || : > "$tmp/base"
      git show ":2:$f" > "$tmp/ours"
      git show ":3:$f" > "$tmp/theirs"
      # Lines inside conflict blocks must all be crate-list lines.
      git merge-file -p "$tmp/ours" "$tmp/base" "$tmp/theirs" > "$tmp/marked" || true
      if awk '
           /^<<<<<<< / { inside = 1; next }
           /^=======$/ && inside { next }
           /^>>>>>>> / { inside = 0; next }
           inside { print }
         ' "$tmp/marked" | grep -vqE "$crate_line"; then
        echo "conflict in $f is not only crate-list lines; aborting" >&2
        rm -rf "$tmp"
        git merge --abort
        exit 1
      fi
      rm -rf "$tmp"
      ;;
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
      # Keep the first of any exactly duplicated crate-list line.
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
