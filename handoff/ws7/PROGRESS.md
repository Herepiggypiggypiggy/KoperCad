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
2026-10-14 21:40 UTC Follow-up PR from main for the remaining #36 review notes: parse_delimited returns rows with file line numbers and errors on an unclosed quote; import_catalog refuses duplicate headers; apply_table reports unparseable typed cells and Excel-formula tags. fmt, clippy, workspace tests clean.
2026-10-14 21:31 UTC WS-02 decided the terminal-article store: Terminal.part only for a terminal's own article; strip default per kind stays terminal_part.<kind> (WS-11); novo_parts::terminal_part stays the single resolver. Nothing to change in novo-parts.
2026-10-14 21:58 UTC Redid #72 (closed) on #81 per WS-02's decision: novo_terminals::terminal_part is the one resolver (Terminal.part, else the strip style), no fallback to the strip device's part; strip row, BOM, novo_parts::missing_parts and the novo-checks missing-part check agree. Gate 1 fixture: -X1 has no device part; its blocks are the strip style (3044076 feed-through, 3044089 PE), so the BOM counts 3 + 1 for 4 terminals. propose_parts skips terminal strips.
2026-10-14 22:05 UTC #99: tester F4 folded in: assign_part of a terminal block (letter code X) to a strip sets the strip's part for the kind it suits (terminal_kind_of: PE block -> PE terminals, else feed-through) via novo_terminals::strip_part_key, never the strip device's part list. Test a_terminal_block_given_to_a_strip_is_ordered_per_terminal; Gate 1 scenario assigns the PE block too.
