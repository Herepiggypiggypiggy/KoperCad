# WS-11 Terminals: handoff (rewrite whole file each time)

Updated: 2026-10-14 17:07 UTC

## State
- Repo: github.com/Herepiggypiggypiggy/NovoElectric. Branch `claude/project-thread-xqeqyw` (the harness-assigned branch name).
- PR #30 "WS-11: terminal strips ...", open. Head 800680b453aba766886881118167047f71f62caa.
- CI: 532d23b was green on Linux, Windows and installer; check the latest head with get_check_runs.
- Review: head SHA sent to the main reviewer; no verdict yet.
- The session is subscribed to PR activity for #30.

## Code
crates/novo-terminals/src: settings.rs (rule, TEMPORARY property keys), naming.rs, occurrence.rs (sides/targets/state; pin 0 is internal, the rest external), strip.rs (rows plus edits), style.rs, place.rs, check.rs, screen.rs, suggest.rs. Tests: tests/terminals.rs (dev-deps novo-symbols and novo-tests). Also edited: a CLAUDE.md crate table row and Cargo.lock.

## Resume
```
git fetch origin && git checkout claude/project-thread-xqeqyw
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p novo-terminals
```
After each push, send_message the reviewer the new head SHA. Check the PR is still open before pushing. No pushes after READY.

## Half-done or waiting
- Switch to WS-02's typed rule (info.rules.terminals) once #35 merges; see ROADMAP Next 1 for the exact semantics. Nothing has started on it yet.

## People (send_message session ids)
- Integration lead/coordinator: session_011PfTJ1QaP1JQJPwXcFnxBE (merges). Main reviewer: cse_016C9nTT8cN7voW8t1NQUG4W
- WS-02 data model: cse_01B1jpjzeDiXvWoBCsv1BWt9 (#35). Workflow/DESIGN.md: cse_01JHYmmTDsVHDL7pEab8MGwy
- WS-06 editor: cse_019siuxbyyRNZRgBBww1KocJ. Tester: cse_01LMMcDMt9qaHXoSXy7iDu58. Full roster: /mnt/project-files/coordination/threads.md

## Decisions and traps
- No novo-core API change; edits happen inside a novo_core::Tx so each is one undo step.
- Defaults: join with a connector; PE terminals named PE, then PE2...; numbering fills gaps; pin 1 sits top or left.
- WS-02 Refuse rejects a duplicate record on insert, so do not create a second record under Refuse.
- The Gate 1 fixture's X1:PE has kind feed-through; the checker flags it on purpose (tester informed).
- Reviewer for PRs: GitHub refuses Tadjiro as reviewer (he is the author); he is assigned instead.
- Keep UI out of #30 (lead rule 4/5).
