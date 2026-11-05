# Help icons: explanation text behind "?" (roadmap)

Tadjiro (2026-11-03): explanation text should be nowhere on screen except behind a small "?" next to the
fold/dock button of its window. Menus, windows, strips, legends and floating text should be almost only settings
and pure results (about 95 % less text). Honesty caveats (model estimate, not validated) stay reachable in the "?"
and keep a tiny inline marker where needed.

Mechanism (done): `crates/novo-app/src/app/window_help.rs`. A window's content calls
`window_help::add(ui.ctx(), "text")`; the "?" appears in the header (docked pane: right of the fold caret;
floating window: left of the dock buttons). `window_help::mark(ui, text)` is the "?" for a strip or card with no header.
Text added with no window being built is dropped, so a strip or card must use `mark`.

Areas (one PR each, from main):
1. 3D / Heat / Hoist / Drive run (first PR, done in part, see HANDOFF.md)
2. Schematic (status bar hints, inspector, palette, pages)
3. Cabinet (strip legend, rules, routes, plate, door)
4. Reports and Commissioning (bench, wire list summaries, checks panel)
5. Simulate and Controllers (Live, PLC, safety, add-ons)
6. Start, Help, App menu, Settings, Learn

Each PR: before/after screenshots at 1366x768 and 1920x1080 from the exact head, names where the user finds the "?",
and `check_text` added to the scenarios of the area.
