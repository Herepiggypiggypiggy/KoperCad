# Outputs thread: handoff

Updated 2026-10-14 21:55 UTC. Owns novo-pdf (WS-09), novo-reports (WS-13),
novo-checks (WS-14). Reviewer: main reviewer cse_016C9nTT8cN7voW8t1NQUG4W.
Merges: integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S.

## State
- #86 merged (sheet names step around real pages; cable screens once).
- Branch `claude/project-thread-14j56i` (the only branch this session may
  push; restart it from main after each merge): PDF prints generated pages
  one PDF page per sheet, `PageScene::set_sheet(SheetLabel)` in novo-canvas
  (additive, offered to WS-03), section bookmarks, #86's doc nits.
- Canvas #93 also edits novo-pdf's page loop (xref notes): whichever merges
  second rebases; the conflict is trivial.
- Local CI only (GitHub Actions is down on billing): fmt, clippy, tests.

## Decisions
- Sheet names: `20` for one sheet, else `20.1..`; family moves to `/`, `-`,
  `_` if any dotted name is a real page's; last resort `'` marks.
- Generated pages reflow (DESIGN.md, #88): bookmarks name what each sheet
  shows (`30  Terminals · =A1+CAB-X1`).
- PDF "Sheet n of m" counts every printed sheet of the project, on every page.
- Terminal articles (lead, 21:47): one answer, novo_parts::terminal_part =
  Terminal.part then strip `terminal_part.<kind>`, no strip-device fallback.
  WS-07 builds it (#72 redo) including bom.rs. Stay off bom.rs's terminal
  handling until that merges. Tester's F4 test waits on it.

## Next
- Feeds and PotentialSystem (#82) in novo-checks; TOC listing generated
  sheets per section; Inter font subset; xlsx Created date.

## Resume
```
git fetch origin && git checkout -B claude/project-thread-14j56i origin/main
export CARGO_TARGET_DIR=~/target CARGO_INCREMENTAL=0
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```
