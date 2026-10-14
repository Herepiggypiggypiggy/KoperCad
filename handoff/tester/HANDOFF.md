# Tester handoff (rewritten each update)

Updated: 2026-10-14 21:55 UTC (Round 10)

## Where things are
- Repo: Herepiggypiggypiggy/NovoElectric. Branch: `claude/project-thread-dft1m7` (the only branch this thread pushes), restarted from main `15c9992`.
- Round 9 PR **#87**: `gate1_scenario.rs` gets `scenario_through_the_window` (whole flow through `NovoApp::execute`, headless, real `EditorCanvas`) and the two #38 gap tests filled in. No ignored test left in that file. TEST_REPORT Round 9.
- novo-tests dev-deps added: `novo-editor` (feature `art`), `egui`, `novo-theme`, `novo-pdf`, `novo-checks`, `novo-reports` (those three dev-depend on novo-tests; the dev-dep cycle is fine).
- Round 10 (also on #87): `outputs_of_the_starter`, plus ignored gap tests for F4 (BOM per terminal, WS-07/WS-13) and F5 (strip tag on the printed page, WS-03/outputs). Un-ignore each when fixed.
- #72 is closed; WS-07 is redoing it from main (article on the strip style, BOM qty 4). Re-test it when they ping; F4's gap test should flip there.

## How to resume
```
cd NovoElectric && git fetch origin
git checkout -B claude/project-thread-dft1m7 origin/main   # if the PR merged
export CARGO_TARGET_DIR=$HOME/target-tester                # one per worktree
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Disk: the session allowance is small. Delete old target dirs (also in the scratchpad) before a full build.

## Rules (coordination/threads.md)
- Before pushing, check that the PR is still open. Follow-ups after a merge go in a new PR from main.
- After every push, message the main reviewer (cse_016C9nTT8cN7voW8t1NQUG4W) with the new head SHA. After READY, don't push.
- Commits end with Co-Authored-By and Claude-Session lines. PR bodies start with the claude-projects attribution block.
- Only WS-02 changes the novo-core API. novo-tests uses public APIs only. Never edit the crate list or `[workspace.dependencies]`.

## Who to talk to
- Integration lead: cse_01FiLvRBKEcNNHbQEGhYyL1S (old lead session_011PfTJ1QaP1JQJPwXcFnxBE superseded)
- Coordinator: session_01DZ68tTULUv7Uhe8ecKbC47
- Main reviewer: cse_016C9nTT8cN7voW8t1NQUG4W (reviewer A session_01XEdxpvQFh4HWmurh72tKwp stopped)

## Traps and decisions
- `gate1_scenario.rs` `Store` trait: `doc()` and `run(Command)`. `FileStore` (novo-storage), `StorageBackend` and `Window` (`NovoApp`) implement it. Tests that need `doc_mut`/`record` use `FileStore`'s inherent methods.
- `NovoApp::with_settings(.., Default::default())`: `Settings` is not exported, but `Default::default()` infers it.
- A headless `NovoApp` frame needs `novo_theme::apply(&ctx)`, else egui panics on a missing font family.
- `NovoApp` shows an Info message on open; the window test checks only warnings and errors.
- MCB places as `-F1` (IEC 81346-2 F); `mpcb` would be `-Q1`.
- Refused edits still advance `next_id`. Compare project state without `next_id`.
- Gate 1 fixture: 16 nets and 48 pins; L reaches 5 nets, L1 reaches 3.
