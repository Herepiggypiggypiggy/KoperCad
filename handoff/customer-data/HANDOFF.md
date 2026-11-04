# Customer data in the public repo

Rule: customer-derived knowledge may inform the product, but names, plots, files and plant codes never go in the public repo.

## Done (this PR)
The machine name of the real-machine test cabinet was in 13 files (CFD code comment, thermal-validation doc, lesson coverage table, five handoff folders, one QA scenario comment, one output file name). All now say "real machine" or "a real machine cabinet". Device counts and watts are kept. Nothing but comments, docs and a file-name mention changed; no code behaviour.

Before: 19 hits in 13 files. After: 0 hits for the old name and its spellings in the tree. Other customer and company names from the private notes (checked 2026-11-03): 0 hits in tree, 0 in history. The earlier plant-code scrub is PR #1057.

## What remains in git history (not touched)
- The old machine name appears in about 10 commits across PR branches (#1305, #1350, #1365, #1368, #1373, #1379 and their branch commits).
- The real plant/function codes scrubbed by #1057 still appear in about 41 commits (docs/DESIGN.md, handoff files, fixtures).
- Two old binaries (`kopercad.exe`, `kopercad-cli.exe`) were once committed; they were on the build branch, now rewritten.
- GitHub keeps every pull request as `refs/pull/N/head`. A normal history rewrite does not remove those; only GitHub Support can purge them. The repo has about 1,400 branches on origin, each a path to old commits. Forks and clones made while public keep the data.

## Options (NOT run; needs Tadjiro's decision)
1. **Leave it.** The data is a machine nickname and some site codes, no plots, no files. Cheapest, no risk to open PRs. Residual: anyone can find them in history.
2. **Targeted rewrite with git-filter-repo** (replace-text for the name and codes, drop the two binaries), then force-push all branches and tags. Every open PR, every thread branch and every clone breaks and must be re-created; about 1,400 branches. Needs a freeze of all threads, a full backup mirror first, and a GitHub Support request to expire the `refs/pull/*` and cached views. Medium effort, real cleanup only after Support acts.
3. **Fresh public repo with squashed history** (keep this one private as the archive). Cleanest result; loses PR history, issue links and the free Actions setup would be redone; release URLs and the launcher script change.
4. **Make the repo private** (stops exposure at once; undoes the free Actions minutes, which depended on public).

Recommendation: 1 now (it is low sensitivity), and if the codes are felt to be sensitive, 3 at a quiet moment. Option 2 is the most disruptive for the amount it achieves because of `refs/pull/*`.

## Guard
Reviewers: grep new PRs for the private names listed in the project's private notes (kept outside the repo on purpose, so this file does not repeat them).
