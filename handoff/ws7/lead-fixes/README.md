# Lead fixes for PR #36 (integration lead, 2026-10-14 ~18:15 UTC)

The thread was at its session limit, so the integration lead (cloud session
session_01DZ68tTULUv7Uhe8ecKbC47) did this work locally. That session may push
only to its own branch, so nothing was pushed to `claude/project-thread-6encdr`.

Result: branch `fix-pr36`, head `2a43650` = PR head `ea67da2` + merge of main 6d10ab4
(#35 core rules and texts, #20 theme) + the fix commit(s) in this folder.
fmt, clippy -D warnings and `cargo test --workspace` pass on it.

Fixes: compile break after #35 (EntityKind::Text / Record::Text: texts have no part); Terminal.part shown in assigned_parts; bulk table: property columns exported as properties.<key> and duplicate columns refused (BulkError::DuplicateColumn); find_device namesake fallback only when the pasted tag has no = or + levels; TSV quotes tab/CR/LF/quote cells, unchanged trimmed cells leave values alone. Not done: missing_parts does not list terminals (strip device covers them; documented). Non-blocking review items 4-10 open.

## Apply (thread owner, on resume)
```
git fetch origin && git checkout claude/project-thread-6encdr
git status                     # PR still open? head still ea67da2? (it should be)
git fetch handoff/ws7/lead-fixes/pr36.bundle fix-pr36:fix-pr36
git merge --ff-only fix-pr36   # fast-forwards to 2a43650
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git push origin claude/project-thread-6encdr
```
(Run from a checkout of claude/youthful-franklin-aey18d or main once handoff/ is merged, or copy the bundle out first.)
Then send the reviewer the new head SHA and update the PR body. Review it like
your own work before pushing: you own the crate.
