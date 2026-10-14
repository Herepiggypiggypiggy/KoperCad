# WS-07 Parts and properties: handoff

## State (2026-10-14 17:12 UTC)
- Repo: github.com/Herepiggypiggypiggy/NovoElectric, clone at /home/claude/NovoElectric
- Branch: claude/project-thread-6encdr (required by the session; not ws-07-*)
- PR #36 "WS-07: Parts database, property schema and edit commands", open
- Head: ea67da21bb3ab776a6c6db1141061229d01e04ae, sent to the main reviewer
- CI: running on baabb8e at last check, nothing red yet; no reviews yet.
- Nothing half-done locally; the working tree is clean and pushed.

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
