# Tester handoff (rewritten each update)

Updated: 2026-10-14 17:20 UTC

## Where things are
- Repo: Herepiggypiggypiggy/NovoElectric. Branch: `claude/project-thread-dft1m7` (the only branch this thread pushes).
- Open PR: **#42** "Tests: Gate 1 scenario through commands, with the known-gap list", head `80dcfe8`, based on main `9512822`. The reviewer has been pinged; no verdict yet. Subscribed to its events.
- #42 adds `crates/novo-tests/tests/gate1_scenario.rs` (passing scenario plus 7 ignored `gap_*` tests naming #34, #38, #37/#29, #35, #30, #36, #28), `e2e_motor_starter.rs`, novo-canvas as a dev-dep, and TEST_REPORT Round 6 (gap table, F2).
- When a gap's PR merges: fill in its `gap_*` test against the new API, remove `#[ignore]`, and update the Round table. This goes in a new PR if #42 is already READY or merged.

## How to resume
```
cd NovoElectric && git fetch origin
git checkout claude/project-thread-dft1m7   # or: git checkout -B claude/project-thread-dft1m7 origin/main
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Test an open PR: create a worktree of origin/main, merge the PR branch (on a Cargo.lock-only conflict take main's lock and let cargo regenerate it), copy `crates/novo-tests` in if missing, run the three commands.
In a container, the canvas GPU tests need `libxkbcommon-x11-0 mesa-vulkan-drivers` (llvmpipe); the app runs under Xvfb.

## Rules (coordination/threads.md)
- Before pushing, check that the PR is still open. A PR can merge while you work; follow-ups then go in a new PR from main.
- After every push, message the reviewer (cse_016C9nTT8cN7voW8t1NQUG4W) with the new head SHA.
- Once the reviewer says READY, don't push to that PR.
- Commits end with Co-Authored-By and Claude-Session lines. PR bodies start with the claude-projects attribution block (copy from a merged PR such as #32).
- Only WS-02 changes the novo-core API. novo-tests uses public APIs only.

## Who to talk to
- Integration lead (merges, assigns): session_011PfTJ1QaP1JQJPwXcFnxBE
- Reviewer: cse_016C9nTT8cN7voW8t1NQUG4W
- WS-01 app shell: cse_01AYuFPxz6eQReQdtzYKLMmW (ProjectBackend/StorageBackend after #34)
- WS-03 canvas: cse_013KCL5cYRtSrq4yndv3Z7TD (F2 reported)
- WS-11 terminals: cse_01AqzMrSLZaG8zin7Yf84RZP (#30)

## Traps and decisions
- Refused edits still advance `next_id`. Compare project state without `next_id`.
- Gate 1 fixture expected counts: 16 nets and 48 pins; L reaches 5 nets, L1 reaches 3 (the overload bridge merges the K1-out and motor nets).
- The fixture sets `pass_through` = library pass-through minus bridges. `fixture_matches_library.rs` guards this.
- #30's test expects X1:PE to be flagged, so the PE-kind fix waits until #30 merges, then flips that test in the same PR.
- F2: in `PageScene`, a terminal's tag label overlaps the next terminal and wins the pick. The e2e test uses `pick_all` until WS-03 fixes this.
- The 60 fps check can't be done in the cloud (llvmpipe only, 19 fps release / 53 ms debug frame).
