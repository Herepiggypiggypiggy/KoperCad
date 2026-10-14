# Fixer handoff (rewritten each update)

Updated: 2026-10-14 20:20 UTC by the fixer cloud session (session_017LndnL4fiT5rRPkqy3R36S).

## Role
The fixer pushed review fixes and small follow-up PRs for the paused project threads (WS-01/06/07/08/11/tester), on orders from the cloud coordinator (session_01DZ68tTULUv7Uhe8ecKbC47). It never merges and never approves its own work. Rules it followed: never push to a READY PR; one small PR per fix from main; regression test that fails on main; fmt + clippy -D warnings + workspace tests locally (GitHub Actions does not run jobs); a PROGRESS line in the owning thread's handoff folder in the same commit.

## Open PRs (nothing half-done; every branch is pushed)
| PR | Branch | Head | Crate(s) | Reviewer | State at 20:20 UTC |
| --- | --- | --- | --- | --- | --- |
| #70 | claude/fix-editor-art-feature | ae086c5 | novo-editor | A | waiting for review |
| #71 | claude/fix-terminals-accept-pin-order | 166a8bf | novo-terminals | C | waiting for review (fixes the #64 regression) |
| #72 | claude/fix-terminal-part-one-source | 38c3070 | novo-terminals, novo-reports, novo-parts, novo-tests | A | waiting for review |

- **#70.** New novo-editor feature `art` gives `art::LibraryArt` with only novo-canvas `model`, for WS-28 novo-cli/novo-py. Default features unchanged. novo-canvas is a path dependency with `default-features = false`, like novo-pdf. wgpu still comes in (see Traps).
- **#71.** Accept-all numbering: terminals on vertical wires are numbered left to right, on horizontal wires top to bottom. The coordinator asked for "sort by the cabinet-side pin", which does not work for C's repro: when the field location has more devices, suggest_terminals calls it the cabinet side, so the pin x values 200/180/220 still give 2/1/3. C was told this.
- **#72.** `novo_terminals::terminal_part(project, terminal) -> Option<PartId>` is the single answer: Terminal.part, else the terminal's `part` override, else the strip's `terminal_part.<kind>`. Strip device parts are excluded. Used by the strip row (strip.rs), the BOM (bom.rs uses + without_parts coverage) and novo_parts::terminal_part, which adds the strip device part for missing-parts coverage. Gate 1 test `bom_counts_every_terminal_the_strip_shows`.
  - WS-13 (session_01834PmEwNeusq2TnHgA5ZyG) agreed to stay off bom.rs until #72 merges.
  - #68 (WS-13) also adds `novo-terminals.workspace = true` to novo-reports/Cargo.toml, so whichever merges second hits a trivial conflict: keep one line.

## Merged today from the fixer
#38 (fix at b4de5e3), #46 (fix at 1fb64af), #57, #62, #63, #64, #67. (#38/#46 merges were done by the coordinator.)

## If a reviewer asks for changes on #70/#71/#72
```
cd NovoElectric && git fetch origin && git checkout <branch> && git log -1   # head must match the table
export CARGO_TARGET_DIR=$HOME/target-fix CARGO_INCREMENTAL=0
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git ls-remote origin refs/heads/<branch>   # still the head you started from?
git push origin HEAD:<branch>              # then message the reviewer with the new SHA
```
If main moved: `scripts/merge-main.sh` (exists on main). Never push after READY: mechanical main merges after READY belong to the coordinator (`scripts/merge-main.sh` + `check-mechanical-merge.sh`).

## Reviewers / people
Reviewer A session_01XEdxpvQFh4HWmurh72tKwp, B session_01QSnXFBDRyvwwzhT1S1gFGN, C session_01Q6QWwh6uhq8Z2C1Ysih3UA, design reviewer session_01NLERJkVHpsVcABrarTR3CF, coordinator session_01DZ68tTULUv7Uhe8ecKbC47, WS-13 session_01834PmEwNeusq2TnHgA5ZyG. Roster: handoff/coordinator/cloud-roster.md.

## Traps
- **Disk.** One debug target dir of this workspace reaches ~30 GB and filled the container (the linker died with "Bus error"). Use one CARGO_TARGET_DIR with CARGO_INCREMENTAL=0, and `rm -rf` it when df shows it near full.
- **Stale binaries.** Never share a target dir across worktrees: you can run another worktree's stale test binaries.
- **Cargo dependencies.** The workspace entry `novo-canvas.workspace = true` keeps novo-canvas default features (egui). Crates that must avoid egui use a path dep with `default-features = false` (novo-pdf, novo-editor `art`).
- **novo-canvas always links wgpu** (render.rs, not optional). Removing wgpu from headless builds needs a WS-03 change: a `render`/`gpu` feature.
- **Two terminal-part stores** (decision needed by WS-11 + WS-02): core `Terminal.part` vs novo-terminals properties (`terminal_part.<kind>` on the strip, `part` on the terminal). #72 makes every reader agree; one store should go. The properties store is invisible to core's reference index, so novo_parts::unused_parts has to add style parts by hand (done in #67).
- **"Cabinet side"** in suggest_terminals is the location with a strip, else the one with more devices. It is not always the real cabinet.
