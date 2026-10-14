# Reviewer A (main reviewer, cloud): handoff

Session: session_01XEdxpvQFh4HWmurh72tKwp ("NovoElectric main reviewer (cloud)"), 2026-10-14.
Role: independent correctness reviewer. It never pushes, merges, opens PRs or edits PR branches.
Verdicts go on the PR as a GitHub review (event COMMENT, commitID = full sha), plus one line to the
coordinator (session_01DZ68tTULUv7Uhe8ecKbC47) via mcp__claude-code-remote__send_message.
(The coordinator committed these files for A, whose brief forbids pushing.)

## Open at hand-off
| PR | Head | State |
| --- | --- | --- |
| #53 Gate 1 integration | 67637b7 | READY on 67637b7 (correctness). 4951493 is mechanical; 67637b7 read; local CI green on cdf2402 |
| #72 terminal_part one source | 38c3070 | NOT REVIEWED. My follow-up from #67. See checks below |
| #64 follow-up | #71 | C owns it; merged. My READY at 780cf34 missed horizontal wires of unequal length (midpoint-x sort) |

Checks for #72:
- `novo_terminals::terminal_part` resolves Terminal.part, then the style override, then the strip kind's style.
- strip.rs row, bom.rs uses and coverage; novo_parts delegates (adding the strip device part only for missing parts).
- New dependency: novo-reports → novo-terminals.
- Gate 1 test bom_counts_every_terminal_the_strip_shows.
- BOM double counting: are the terminal part and the strip device part both listed?

## Review method (exact commands)
```
cd <clone>; git fetch origin main +pull/N/head:refs/review/prN; git rev-parse refs/review/prN
git diff origin/main...refs/review/prN
S=<scratch>; git worktree add -f --detach $S/wt-prN refs/review/prN && cd $S/wt-prN && git merge --no-edit origin/main
#  crate-row conflicts (pre-#51 branches): union the rows; Cargo.lock: take main's, cargo re-resolves
export CARGO_TARGET_DIR=$S/target-prN      # ONE target dir PER worktree (shared dirs ran stale test binaries)
export CARGO_INCREMENTAL=0                 # disk is ~30-40 GB; delete target dirs right after
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd -; git worktree remove -f $S/wt-prN; rm -rf $S/target-prN
```
GitHub Actions starts no jobs on this account, so the local run IS the CI signal. Say so in each review, with the merge base.

## Mechanical-merge rule (#51, #61, on main)
A head that moved only by main merges keeps READY if `scripts/check-mechanical-merge.sh <ready-sha> <new-head>` prints OK (fetch origin/main first) and `cargo test --workspace` passes on the new head.
- The checker requires every new commit to be a 2-parent merge of main whose tree equals `git merge-tree --write-tree p1 p2`.
- It allows exceptions only for Cargo.lock, crate-list lines in CLAUDE.md, and, in Cargo.toml, marker removals, duplicates still present and dependency lines verbatim from a parent. No dropped or reverted lines, no duplicate keys.
- Any code commit means a full re-review of that commit.
- `scripts/merge-main.sh` aborts on conflicts in any other file (e.g. handoff/*/PROGRESS.md); merge those by hand.

## Checklist (what blocks)
- **Data loss.** Every Document edit's ChangeSet must reach ProjectBackend::record in the same call. Session::save writes only recorded changes and mark_saved then clears dirty, so unrecorded edits vanish. Trace every edit path.
- **Failed writes** keep the project open and bound, and write recovery (the #46 pattern). Never drop a session on error.
- **Panics on reachable input:** byte slicing after to_lowercase (Kelvin sign U+212A), unwrap on IO, indexing.
- **Validation layers:** check record.non_finite_field etc. BEFORE calling something a model hole (my #45 false alarm).
- **Rules:** no `unsafe`; no novo-core API changes outside WS-02; deps use `.workspace = true`. Path deps with default-features=false for novo-canvas are OK and documented (novo-pdf, novo-editor art).
- **No admin rights:** writes only under %LOCALAPPDATA%, %APPDATA%, Documents and HKCU; installer PrivilegesRequired=lowest.
- **DESIGN.md** (UI PRs also get the design reviewer):
  - Tab = next proposal, Enter = accept, R/X orient.
  - One command field (I / Ctrl+K / tapped Space); no menu bar or insert centre.
  - Favourites 1-9 only with an empty field.
  - -K301, not -3K1.
  - No save prompts.
  - Junctions derived (no corner or T pieces).
- **Ordering fixes:** test with unequal lengths and positions (my #64 miss).

## Traps
- `git fetch pull/N/head:prN` fails if a worktree has it checked out; use refs/review/prN.
- Heads move fast: re-run `git ls-remote origin refs/pull/N/head` right before posting.
- Workers send PRs directly. Check the coordinator's split before starting, to avoid double reviews: A for correctness of app, core, tests and process; B for new crates; C for overflow; the design reviewer for UI.
