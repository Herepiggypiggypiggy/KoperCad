# Outputs thread: handoff

Updated 2026-10-15 02:55 UTC. Owns novo-pdf (WS-09), novo-reports (WS-13),
novo-checks (WS-14). Reviewer: main reviewer cse_016C9nTT8cN7voW8t1NQUG4W.
Merges: integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S.

## State
- Merged: #86 (sheet names step around pages, cable screens once), #96 (PDF
  prints one page per report sheet, section bookmarks).
- #115 merged (sheet letters).
- Checks PR (from checks-feeds) open on the branch; see PROGRESS for its number.
- Local branch `checks-feeds` (94e8e89, on main after #99): optional spare
  pins, feeds (Net.sources) in shorts, UndersizedWire (IEC 60204-1 B2 vs a
  protective device's rated_current_a), WrongTerminalBlock (PE terminal vs
  catalog PE marker via novo_terminals::terminal_part; parts with no type
  number or keywords are skipped). Broken starter has 12 errors, (3,8,1).
- Local branch `pdf-sheet-content` (ab1064a, on top of #115): generated
  sheets go through PageScene::set_sheet (#109); title block shows `20b` and
  `2 of 3` within the page; `novo_pdf::sheet_content` is public for the app's
  canvas (no caller of set_sheet in novo-app yet: WS-01). Also the #116 doc
  nit on sheets.rs. Bold headers and per-cell clipping are gone (canvas
  ellipsis-fits instead).
- Local branch `toc-sheets` (a13ccec): the contents list has one row per
  generated sheet, titled with what it shows; toc depends on what the other
  reports read.
- One branch only: after the checks PR, pdf-sheet-content, then toc-sheets.
- Local CI only (Actions down on billing). Disk is tight: use
  CARGO_INCREMENTAL=0, `rm -rf target` when full.

## Decisions
- Sheet names (DESIGN #108/#110): `20a..20z, 20aa`, no i/l/o, never a dot.
- n of m (DESIGN #106): within each generated page; schematic pages count
  schematic pages only.
- Terminal articles: one answer, novo_terminals::terminal_part (#99).

## Next
- TOC listing generated sheets per section; Inter font subset; xlsx Created
  date; overlap of `20aa` with a page named `20a`'s sheets (far-fetched).

## Resume
```
git fetch origin && git checkout -B claude/project-thread-14j56i origin/main
export CARGO_INCREMENTAL=0
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```
