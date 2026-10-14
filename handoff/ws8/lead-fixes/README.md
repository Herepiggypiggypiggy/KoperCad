# Lead fixes for PR #29 (integration lead, 2026-10-14 ~18:15 UTC)

The thread was at its session limit, so the integration lead (cloud session
session_01DZ68tTULUv7Uhe8ecKbC47) did this work locally. That session may push
only to its own branch, so nothing was pushed to `claude/project-thread-9lkslj`.

Result: branch `fix-pr29`, head `c3f4d7f` = PR head `946d8ed` + merge of main 6d10ab4
(#35 core rules and texts, #20 theme) + the fix commit(s) in this folder.
fmt, clippy -D warnings and `cargo test --workspace` pass on it.

Fixes: undo/redo yield to text fields (new Scope::OutsideText); copied main poles get a new device when the coil is not copied (aux contacts stay on the old device); smart duplicate copies attached Text records; journeys test expects one net fewer because the copied -X1:PE joins the original under the Join rule (correct: PE is one conductor); natural tag order; view.focus_mode renamed view.focus; view.pin_outline added. Open, non-blocking: cables in palette search, two command tables (WS-01), Space tap vs hold, Escape in text fields, renumber vs next_tag grouping, Keymap::rebind &'static str.

## Apply (thread owner, on resume)
```
git fetch origin && git checkout claude/project-thread-9lkslj
git status                     # PR still open? head still 946d8ed? (it should be)
git fetch handoff/ws8/lead-fixes/pr29.bundle fix-pr29:fix-pr29
git merge --ff-only fix-pr29   # fast-forwards to c3f4d7f
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git push origin claude/project-thread-9lkslj
```
(Run from a checkout of claude/youthful-franklin-aey18d or main once handoff/ is merged, or copy the bundle out first.)
Then send the reviewer the new head SHA and update the PR body. Review it like
your own work before pushing: you own the crate.
