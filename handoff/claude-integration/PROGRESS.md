2026-10-20 21:05 UTC started thread "Claude integration core" (limit reset, Tadjiro: work autonomously).
2026-10-20 21:40 UTC novo-ai crate written, 41 offline tests green, clippy clean.
2026-10-20 22:05 UTC app window, command, settings, app tests with scripted transport added.
2026-10-20 21:55 UTC Lead A review 1: scope narrowing clears chat, outline/NOTICE wording, key perms, second agree for wider scope, UI nits.
2026-10-21 08:10 UTC Issues rows: 'Explain with Claude' (context menu) via ask_later.
2026-10-21 07:40 UTC Ctrl+K: no match offers 'Ask Claude: <text>' when Claude is on.
2026-10-21 05:50 UTC #1064 merged. Streaming replies (stream.rs, Client::messages_stream, Stop button) in a new PR; try-with-real-key steps in PR #1070.
2026-10-21 06:20 UTC edit actions: assign_part, set_property, add_page (PR from claude/ai-edit-actions).
2026-10-21 07:00 UTC #1070, #1076, #1081, #1088, #1090 merged. Heat panel: Describe with Claude (report paragraph), Copy on answers (PR from claude/ai-heat).
2026-10-21 07:10 UTC picture of the window attached by the person (Attach picture), PNG, shrunk to 1400px, sent once.
2026-10-21 07:20 UTC #1120 merged. Claude can read the design checks (read_project section problems, Whole project only) (PR from claude/ai-checks-tool).
2026-10-21 10:10 UTC Part from a datasheet (Help > Ask Claude > "Part from a datasheet"): PDF sent as a document block, report_part tool with page and quote per field, person ticks fields, one undo step. Consent version 4. Decision: no in-app click tools for Claude (they would bypass Apply); the control harness stays for tests only. Open request for Tadjiro: real-key test, steps in docs/CLAUDE-ASSISTANT-TEST.md.
2026-10-21 07:40 UTC #1129 merged. Claude can read wire-number conflicts (read_project section wire_numbers, Whole project only; consent version 3) (PR from claude/ai-numbering-tool).
2026-10-21 10:40 UTC #1136 and #1165 merged. Follow-up: datasheet confirm line shows cost and month spend, Stop while reading, warning for a duplicate manufacturer and order number (PR from claude/ai-datasheet-followup).
2026-10-21 11:00 UTC #1170 merged. Polish: spend label says counted by KoperCAD, no dollar guess, duplicate match ignores case/spaces/dashes, Stop tested (PR from claude/ai-datasheet-polish).
2026-10-21 12:45 UTC Claude can propose set_wire_number (Whole project only, card shows the old number, one undo step) so it can act on the wire-number conflicts it reads (PR from claude/ai-wire-number-edit).
