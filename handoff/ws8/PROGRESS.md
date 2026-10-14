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
