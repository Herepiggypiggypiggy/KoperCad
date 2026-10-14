# Tester handoff (rewritten each update)

Updated: 2026-10-14 22:20 UTC (Round 10)

## Where things are
- Repo: Herepiggypiggypiggy/NovoElectric. Branch: `claude/project-thread-dft1m7` (the only branch this thread pushes).
- Open PR **#87** (Rounds 9-10, tests only): Gate 1 through `NovoApp::execute`, the #38 gap tests, `outputs_of_the_starter` (checks, xrefs, BOM incl. one terminal block per terminal, PDF), F4 gap tests (both pass since #99). Main reviewer said READY on 2df3673; the main merge after #99 needs a delta check.
- Only ignored test left in `gate1_scenario.rs`: `gap_pdf_names_the_terminal_strip` (F5). WS-03 has the fix queued after #103; un-ignore it in a new PR from main once that merges.
- F4 closed by #99. The PE-article design check (PE terminal with a non-PE block) went to the outputs thread (novo-checks).
- O1 (mcb3 tagged -F1 vs -Q1) is still an open WS-08 question; tests pin current behaviour.

## How to resume
```
cd NovoElectric && git fetch origin
git checkout -B claude/project-thread-dft1m7 origin/main   # if #87 merged
export CARGO_TARGET_DIR=$HOME/target-tester                # one per worktree
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
GitHub Actions is down (billing), so checks run locally. Disk allowance is small: delete old target dirs (also in the scratchpad) before a full build.

## Rules (coordination/threads.md)
- Before pushing, check the PR is still open and **not READY**. After READY, don't push unless the lead asks; follow-ups go in a new PR from main.
- After every push, message the main reviewer (cse_016C9nTT8cN7voW8t1NQUG4W) with the new head SHA.
- Commits end with Co-Authored-By and Claude-Session lines. PR bodies start with the claude-projects attribution block.
- Only WS-02 changes the novo-core API. novo-tests uses public APIs only. Never edit the crate list or `[workspace.dependencies]`; new novo-tests deps use `.workspace = true`.

## Who to talk to
- Integration lead: cse_01FiLvRBKEcNNHbQEGhYyL1S
- Main reviewer: cse_016C9nTT8cN7voW8t1NQUG4W
- WS-07 parts: cse_01H3m3ufThxMEzTtG41NrXaB; WS-03 canvas: cse_013KCL5cYRtSrq4yndv3Z7TD; Outputs (PDF/reports/checks): cse_01GNLWH7SKcFppZwCxZoC1rw

## Traps and decisions
- `gate1_scenario.rs` `Store` trait: `doc()` and `run(Command)`; `FileStore`, `StorageBackend` and `Window` (`NovoApp`) implement it.
- `starter_with_parts` gives devices their catalog part and -X1's terminals their articles via `assign_terminal_part` (UT 2,5 feed-through, `pe` query for UT 2,5-PE).
- `NovoApp::with_settings(.., Default::default())`; a headless frame needs `novo_theme::apply(&ctx)`.
- PDF text: terminals print only their names; `/Type /Page\n` counts page objects (`/Type /PageLabel` and `/Pages` also match the bare prefix).
- Refused edits still advance `next_id`. Gate 1 fixture: 16 nets, 48 pins; since #99 the fixture keeps -X1's blocks in the strip style, not its part list.
