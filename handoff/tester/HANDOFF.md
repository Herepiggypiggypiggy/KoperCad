# Tester handoff (rewritten each update)

Updated: 2026-10-14 (Round 9)

## Where things are
- Repo: Herepiggypiggypiggy/NovoElectric. Branch: `claude/project-thread-dft1m7` (the only branch this thread pushes), restarted from main `15c9992`.
- Round 9 PR (see PROGRESS for its number): `gate1_scenario.rs` gets `scenario_through_the_window` (whole flow through `NovoApp::execute`, headless, real `EditorCanvas`) and the two #38 gap tests filled in. No ignored test left in that file. TEST_REPORT Round 9.
- novo-tests dev-deps added: `novo-editor` (feature `art`), `egui`, `novo-theme`.
- BOM terminal count check: #72 (open, fixer) already adds `bom_counts_every_terminal_the_strip_shows`; not duplicated.

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
- After every push, message reviewer A (session_01XEdxpvQFh4HWmurh72tKwp) with the new head SHA. After READY, don't push.
- Commits end with Co-Authored-By and Claude-Session lines. PR bodies start with the claude-projects attribution block.
- Only WS-02 changes the novo-core API. novo-tests uses public APIs only. Never edit the crate list or `[workspace.dependencies]`.

## Who to talk to
- Integration lead: session_011PfTJ1QaP1JQJPwXcFnxBE
- Coordinator: session_01DZ68tTULUv7Uhe8ecKbC47
- Reviewer A: session_01XEdxpvQFh4HWmurh72tKwp

## Traps and decisions
- `gate1_scenario.rs` `Store` trait: `doc()` and `run(Command)`. `FileStore` (novo-storage), `StorageBackend` and `Window` (`NovoApp`) implement it. Tests that need `doc_mut`/`record` use `FileStore`'s inherent methods.
- `NovoApp::with_settings(.., Default::default())`: `Settings` is not exported, but `Default::default()` infers it.
- A headless `NovoApp` frame needs `novo_theme::apply(&ctx)`, else egui panics on a missing font family.
- `NovoApp` shows an Info message on open; the window test checks only warnings and errors.
- MCB places as `-F1` (IEC 81346-2 F); `mpcb` would be `-Q1`.
- Refused edits still advance `next_id`. Compare project state without `next_id`.
- Gate 1 fixture: 16 nets and 48 pins; L reaches 5 nets, L1 reaches 3.
