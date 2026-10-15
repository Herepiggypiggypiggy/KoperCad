# WS-08 progress log
2026-10-14 16:24 UTC Thread started; asked the Workflow thread for change requests.
2026-10-14 16:34 UTC Opened PR #29: novo-workflow crate (keymap, commands, palette, insert, orientation, history, duplicate).
2026-10-14 16:41 UTC Pushed 5d25f14: rebuilt on novo-core search and xrefs (#26), added renumber, dropped J pieces and escape ladder.
2026-10-14 16:47 UTC Pushed 3df7c90: interaction-model keys and command field grammar (arbitration: #24 model wins).
2026-10-14 16:51 UTC Pushed edf3bbf: switched to novo-symbols by_alias/favorites (#25).
2026-10-14 16:55 UTC Pushed 01d8023: drawer keys Ctrl+3/4/5, focus mode, F11; every grammar form pickable by rows. Told the reviewer; holding #29.
2026-10-14 17:00 UTC Wrote the handoff files.
2026-10-14 17:01 UTC Agreed the split with WS-06: novo-editor will use novo-workflow's insert search, next_tag, duplicate and keymap (API names sent).
2026-10-14 17:05 UTC DESIGN.md merged. Diffed the keys against #29: Shift+O and ? are missing (follow-up). #29 stays frozen.
2026-10-14 17:07 UTC Workflow thread: next PR adds Dutch aliases for verbs, logical-key handling and Tab consumption; ping them with a head.
2026-10-14 17:12 UTC WS-01 is renaming its action ids: focus mode is "view.focus" (mine is view.focus_mode) and it adds view.pin_outline. Align these in the follow-up PR.

- 2026-10-14: CI fmt check red on 01d8023 (journeys.rs). Pushed 946d8ed (cargo fmt only). Reviewer and coordinator told.
2026-10-14 18:15 UTC Integration lead fixed PR #29 locally (review items + #35 merge) at c3f4d7f; bundle and patches in lead-fixes/, not pushed.
2026-10-14 UTC (fixer session_017LndnL4fiT5rRPkqy3R36S): fixes from the Gate 1 integration run: `accept all` ran `accept` with argument `all` (the longest typed verb phrase now wins); the device list after `no ` sorted -K10 before -K2 (natural tag order now; devices with a coil still first; DESIGN.md gives no other order). PR claude/fix-workflow-accept-all.
- 2026-10-14 21:50: #29 merged. Follow-up branch restarted from main: MPCB as Q, ? and Shift+O, Ctrl+Y first, Escape outside text, Dutch command words, Keep-mode terminal names, sub-device renumber, split_verb fix. fmt, clippy -D warnings and cargo test --workspace pass (Linux).
- 2026-10-14 22:30: #90 merged (97c9e34). New PR from main: renumber keeps -K7 with -K7.1 (duplicate plain tags still split); `page 2` / `pagina 2` jump to the page (no bare page/pagina verb on page.new). Letter-code table unification proposed to the lead (novo-symbols owner), not done here.
