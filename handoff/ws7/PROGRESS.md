# WS-07 progress (append-only)

2026-10-14 16:24 UTC Thread started; read roadmap, WORKFLOW.md (PR #24) and novo-core API
2026-10-14 16:26 UTC Asked workflow thread for change requests
2026-10-14 16:40 UTC Pushed b7d1353: novo-parts crate (catalog, assignment, property sheet, bulk edit, CSV)
2026-10-14 16:43 UTC Pushed 350de50: egui panel + bulk editor; opened PR #36
2026-10-14 16:46 UTC Decision: removed the egui panel per Tadjiro's "own program" direction (relayed by integration lead)
2026-10-14 16:46 UTC Pushed baabb8e: part overrides + reset_to_part, fill(), Catalog::preview_merge
2026-10-14 16:50 UTC WS-02 added Terminal.part in #35; to adopt after it merges
2026-10-14 16:58 UTC Sent head baabb8e to main reviewer; CI queued
2026-10-14 17:00 UTC Handoff files written
2026-10-14 17:12 UTC DESIGN.md on main; pushed ea67da2: propose_parts, auto_assign_parts, missing_parts_summary; head sent to reviewer
2026-10-14 17:13 UTC Workflow thread: #36 at ea67da2 meets DESIGN.md criteria, no blockers; editor (WS-06) to call auto_assign_parts on sizing changes
2026-10-14 18:15 UTC Integration lead fixed PR #36 locally (review items + #35 merge) at 2a43650; bundle and patches in lead-fixes/, not pushed.
2026-10-14 UTC (fixer session_017LndnL4fiT5rRPkqy3R36S): claude/ws07-followups from main: #36 review follow-ups (part cells change only through the Order number and report ignored Type number / Manufacturer edits in BulkOutcome.ignored; set_cell/fill refuse Manufacturer/Type number alone; a second row for one device is skipped; properties.<key> headers resolve to the project's key or the normal form; parts listed once) and terminal parts (assign_terminal_part, terminal_part, missing_parts with terminals, unused_parts keeps strip style parts). novo-parts now depends on novo-terminals.
2026-10-14 UTC (fixer session_017LndnL4fiT5rRPkqy3R36S): one answer for a terminal's article: novo_terminals::terminal_part (Terminal.part, else style override, else strip style for the kind) used by the strip row, the BOM (novo-reports) and novo_parts::terminal_part (which adds the strip device part for coverage). Gate 1 test: BOM terminal count = strip rows. PR claude/fix-terminal-part-one-source.
