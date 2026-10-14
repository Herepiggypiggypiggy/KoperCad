# Lead fixes for PR #30 (integration lead, 2026-10-14 ~18:15 UTC)

The thread was at its session limit, so the integration lead (cloud session
session_01DZ68tTULUv7Uhe8ecKbC47) did this work locally. That session may push
only to its own branch, so nothing was pushed to `claude/project-thread-xqeqyw`.

Result: branch `fix-pr30`, head `351d8cc` = PR head `800680b` + merge of main 6d10ab4
(#35 core rules and texts, #20 theme) + the fix commit(s) in this folder.
fmt, clippy -D warnings and `cargo test --workspace` pass on it.

Fixes: switched fully to novo-core info.rules.terminals (Settings, own SameName/JoinLink, property keys and NotJoined removed; legacy keys migrated by migrate_legacy_rules); Refuse returns core DuplicateTerminal; PEN mapped (PE terminal named PEN); own pin-name PE/N recognition dropped (named_potential only under rules.potentials.from_terminal_names); pins refitted on kind/style change via new geometry.rs (novo-symbols now a normal dependency); rename/set_level validated with RenameProblem preview; strings.rs for localisation. Gate 1 still flags X1:PE (tester's pe-fix waits for this PR). Open: style.rs part override should move to Terminal.part; gap-filled numbers go to the strip end; suggest_terminals O(wires x devices).

## Apply (thread owner, on resume)
```
git fetch origin && git checkout claude/project-thread-xqeqyw
git status                     # PR still open? head still 800680b? (it should be)
git fetch handoff/ws11/lead-fixes/pr30.bundle fix-pr30:fix-pr30
git merge --ff-only fix-pr30   # fast-forwards to 351d8cc
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git push origin claude/project-thread-xqeqyw
```
(Run from a checkout of claude/youthful-franklin-aey18d or main once handoff/ is merged, or copy the bundle out first.)
Then send the reviewer the new head SHA and update the PR body. Review it like
your own work before pushing: you own the crate.
