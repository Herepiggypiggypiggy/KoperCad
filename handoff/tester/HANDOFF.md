# Tester handoff (rewritten each update)

Updated: 2026-10-15 08:15 UTC (Round 15)

## Where things are
- Repo: Herepiggypiggypiggy/NovoElectric. Branch: `claude/project-thread-dft1m7` (the only branch this thread pushes), restarted from main `f3534dc` after #87 merged.
- Gate 1: no open gap. #87 merged (window scenario, outputs, F4), #99 closed F4, #126 closed F5.
- Gate 2: met (Round 15, main `84fefd2`, 826/0/2). #172 closed G2-1..G2-3; their tests run now.
- Wire overlap (Round 14, #173 merged): `wire_overlap.rs`. Engine PR 1 is #181 (`novo_editor::overlap::{page_overlaps, route_overlaps, plan_route, Refusal}`); Lines lands in engine PR 2. Next: Corners journey (wiring-modes test 2), Lines journey (test 3), drop the same-net filter in the guard, interactive move form, `wire.overlap` script-path test.
- Windows: CI runs on main since #175; check the first result and the real build with the release thread.
- When a gap's fix merges: un-ignore its test in a new PR from main.

## How to resume
```
cd NovoElectric && git fetch origin
git checkout -B claude/project-thread-dft1m7 origin/main   # if the PR merged
export CARGO_TARGET_DIR=$HOME/target-tester
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
GitHub Actions is down (billing), so checks run locally. Disk allowance is small: a full build needs ~15 GB; delete the target dir when a build fails with "No space left".

## Rules (coordination/threads.md)
- Before pushing, check the PR is still open and not READY. After READY, don't push unless the lead asks; follow-ups go in a new PR from main.
- After every push, message the main reviewer (cse_016C9nTT8cN7voW8t1NQUG4W) with the new head SHA.
- Commits end with Co-Authored-By and Claude-Session lines. PR bodies start with the claude-projects attribution block.
- novo-tests uses public APIs only, deps with `.workspace = true`. Never edit the crate list or `[workspace.dependencies]`.

## Who to talk to
- Integration lead: cse_01FiLvRBKEcNNHbQEGhYyL1S
- Main reviewer: cse_016C9nTT8cN7voW8t1NQUG4W
- WS-07 parts: cse_01H3m3ufThxMEzTtG41NrXaB; WS-03 canvas: cse_013KCL5cYRtSrq4yndv3Z7TD; Outputs (PDF/reports/checks): cse_01GNLWH7SKcFppZwCxZoC1rw; WS-01 app shell: cse_01AYuFPxz6eQReQdtzYKLMmW

## Traps and decisions
- `gate1_scenario.rs`: `Store` trait (`doc()`, `run(Command)`) over `FileStore`, `StorageBackend` and `Window` (`NovoApp`). A headless `NovoApp` frame needs `novo_theme::apply(&ctx)`.
- Cable overview: `spare` counts cores marked spare (`cable.spare`), not unused ones; `used` counts cores with a wire.
- PDF text checks: strings are uncompressed `(text)` operands; `/Type /Page\n` counts page objects.
- Refused edits still advance `next_id`. Gate 1 fixture: -X1's blocks are in the strip style since #99.
- O1 (MCB as -F1 or -Q1) is WS-08's call; tests pin current behaviour.
