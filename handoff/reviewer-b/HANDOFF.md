# Main reviewer B: handoff (2026-10-14 20:30 UTC, cloud wrap-up)

## Who I am
Main reviewer B. I review and never write product code: no pushes, no merges and no PRs. The only exception is this handoff PR, which the coordinator asked for. I took the new-crate PRs and worker PRs. Policy since 19:30 UTC: every worker new-crate PR gets BOTH reviewer B and reviewer C, and either one's CHANGES NEEDED blocks the merge. Verdicts go to the coordinator (cloud session_01DZ68tTULUv7Uhe8ecKbC47; on device, see `/mnt/project-files/coordination/threads.md`). For worker PRs the verdict also goes to the worker's session. The general rules (what counts as blocking, the DESIGN.md list) are in `handoff/reviewer/HANDOFF.md`; read that first.

## State of every PR I touched (all reviews are posted on GitHub as COMMENT reviews)
| PR | What | Last verdict from me | State |
|---|---|---|---|
| #49 novo-pdf (WS-09) | vector PDF | READY 95d4749; merge heads b930144 and ad4fd1e confirmed mechanical | merged 27f0e96 |
| #50 novo-checks (WS-14) | design checks | CHANGES NEEDED 662086e (example `scale` collided with novo-core's), READY f72ab36 | merged 6ec72c4 |
| #54 novo-cables (WS-12) | cables | READY 42a921b | merged efac4d4 |
| #55 novo-reports (WS-13) | reports | CHANGES NEEDED 6d40835 (CSV formula injection), READY 37f684f / 215af04 / 3a6771c (deterministic xlsx) | merged ea47b10 |
| #56 novo-cli + novo-py (WS-28) | CLI + Python | READY 19c1f00, withdrawn after C found recovery-file deletion; READY ac037dc, e5bb8ae, 812d371 | merged 67c3591 |
| #59 xref example rename | fix of my finding | READY 4d87cf8 | merged 8243d22 |
| #65 WS-28 follow-ups | check/pdf/report exports | READY cb47f73, then READY 3354d4e (pushed after READY; flagged) | OPEN at 3354d4e; needs C |
| #68 WS-13 follow-ups | terminal/cable diagrams, sheets | READY 02d6e29 | OPEN at 02d6e29; needs C |
| #70 novo-editor `art` feature (fixer) | LibraryArt without egui | READY ae086c5; head ec6f2ff = ae086c5 + main 9839410, checked mechanical (PR files identical) | OPEN at ec6f2ff |

Nothing is half-done. No review is in flight.

## Resume (review a PR)
```
git fetch origin main +pull/N/head:refs/review/prN && git rev-parse refs/review/prN
S=<scratch>; git worktree add -q --detach $S/wtN refs/review/prN && cd $S/wtN
git merge --no-edit origin/main          # Cargo.toml dep-tail conflicts: keep both sides; Cargo.lock: take main's, then `cargo metadata`
export CARGO_TARGET_DIR=$S/tgtN CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
# novo-py: cargo clippy -p novo-py --features python --all-targets -- -D warnings
#   pytest: build --features python --lib, copy target/debug/libnovo_py.so to <dir>/novoelectric.abi3.so,
#   python3 -m venv v && v/bin/pip install pytest && PYTHONPATH=<dir> v/bin/python -m pytest crates/novo-py/python/tests
# delete worktree + target dir afterwards (disk)
```
Mechanical-merge check for a head that only merged main: compare `git diff <main-parent> <head> --stat` with the PR's own file list, `git diff <old READY head> <head> -- <PR crates>` (must be empty), and the Cargo.lock +/- lines (must be identical).

## Traps learned today
- GitHub Actions runs no jobs on this account (runner_id 0). The local fmt, clippy and test run is the CI signal; say so in every review.
- Example names must be unique across the workspace (cargo "output filename collision", cargo#6313). Check with `git ls-tree -r --name-only HEAD crates | grep /examples/`.
- Look for time-dependent byte comparisons (the xlsx creation timestamp made #55 flaky) and wall-clock assertions in debug tests.
- `ProjectFile::create_as` deletes the target's `.autosave`. Any "save as" or "create" path must refuse targets with a recovery file (C's #56 finding).
- When Excel opens a CSV, it evaluates any field starting with = + - @ as a formula. IEC tags start with = + -.
- Since #51, main pre-registers every crate. PR merges conflict only on external dependency lines; keep both.
- `ExcelDateTime`, `rust_xlsxwriter` 0.99: `created` and `modified` both come from `DocProperties::creation_time`.
