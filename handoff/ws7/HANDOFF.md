# WS-07 Parts and properties: handoff

## State (2026-10-14 21:40 UTC)
- #36 and #67 are merged. Branch claude/project-thread-6encdr was restarted
  from main for the last #36 review notes (CSV parsing, line numbers,
  duplicate catalog headers, typed cells, Excel formula tags); see PROGRESS.
- CI (Actions) is down; reviewers' local fmt + clippy + tests are the signal.
- Terminal-article store decided by WS-02 (21:31 UTC): Terminal.part only; strip default terminal_part.<kind>; novo_parts::terminal_part is the resolver.

## What is in the PR
- crates/novo-parts/: catalog.rs, search.rs, assign.rs, sheet.rs, table.rs,
  csv.rs, catalog/starter.json, tests/parts.rs (18 integration + 5 unit tests)
- Proposals (DESIGN.md §2): propose_parts, auto_assign_parts (only when exactly
  one part fits), missing_parts_summary (Issues line)
- Cargo.toml: novo-parts in [workspace.dependencies]; CLAUDE.md crate row
- No novo-core changes. All edits return novo_core::Command for
  Document::execute (one undo step).

## Resume
```
cd /home/claude/NovoElectric && git fetch origin && git checkout claude/project-thread-6encdr
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```
Before pushing: check that PR #36 is still open and the reviewer has not said
READY. After each push, send the new head SHA to the main reviewer.

## People (coordination/threads.md)
- Integration lead/coordinator: session_011PfTJ1QaP1JQJPwXcFnxBE (merges)
- Main reviewer: session_016C9nTT8cN7voW8t1NQUG4W
- Workflow / DESIGN.md: session_01JHYmmTDsVHDL7pEab8MGwy (sent all of its criteria; it is happy)
- WS-02 data model: session_01B1jpjzeDiXvWoBCsv1BWt9 (added Terminal.part in #35)
- WS-01 app shell: session_01AYuFPxz6eQReQdtzYKLMmW (told: no panel until DESIGN.md)

## Decisions and traps
- docs/DESIGN.md is on main; §7 = part is the style of a device. Not done:
  persisting declined proposals (suggested a declined_parts property)
- No UI until DESIGN.md merges (Tadjiro: own program, not EPLAN-like).
- Part data stays on the project Part; per-device values are overrides
  (Row::overrides_part, reset_to_part). The idea is going into DESIGN.md §7.
- Catalog import is not in project undo (the catalog lives outside the project); preview_merge first.
- Batch inserts of new parts use ids from project.next_id() counting up
  (PartAllocator); keep that if you add more inserts to a batch.
- stress_page(doc, n) makes n/3 devices, not n.
- Starter catalog is example data; order numbers should be verified.
- Bool/num parsing accepts decimal comma; types follow the existing value or part value.

## Lead fixes waiting (read first)
See lead-fixes/README.md: review items and #35 fallout fixed locally at 2a43650 by the integration lead; ALREADY PUSHED at 18:06 UTC; just `git pull` the PR branch.
