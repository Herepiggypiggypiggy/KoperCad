# Visual redesign lane V3: handoff

Updated 2026-10-29. Branch `claude/project-thread-up7jgz`, based on main 8c9c97e (V0 merged).

## What exists
- `rail.rs`: generic widget. `Rail { tools, assists, panels, history }` of `Entry` (id, anchor, glyph, label, tip, key, active, enabled, badge, flyout). `rail::show(ui, &Rail, &mut expanded) -> Shown`. Flyout state lives in egui temp data (`rail::open_flyout`). Esc or an outside click closes it, a second click on the entry toggles it. Up/Down arrows move between entries; focus ring is a 2 pt outline; High contrast draws a border on every entry. No animation, so Reduce motion needs no branch.
- `rail_schematic.rs`: row lists `TOOLS`, `ASSISTS`, `PANELS`, `HISTORY` (command ids, or `rail.*` for rows that are not commands), `look(id)` (glyph, short label, sentence from the V0 presentation table), `schematic_rail()`, `schematic_rail_show()`, `rail_click()`.
- The old `tool_dock` is gone (`dock.rs` keeps `tool_hint` and the editor sync functions).
- Setting `rail_expanded` (foot button); Preferences "Dock labels = Always" also forces expanded.
- Lesson anchors `dock.*` are kept on the rail rows so lessons keep working; `dock.corners` and `dock.lines` sit in the Connect flyout and fall back to Connect while it is shut.

## Traps
- Rail height: 18 rows at 34 pt must fit 704 pt (1366x768); `the_rail_fits_without_scrolling...` guards it. A new row in a zone costs 36 pt; max 6 per zone.
- In tests a click needs a move frame, a settle frame, then press (hover is from the previous frame).
- `command_unavailable` is now `pub(super)` in menu_bar.rs; Accept proposals is greyed when no proposals wait.
- The rail is a real left panel, so the canvas shifts right; the sim status chip (#1637) lives on the 3D and Cabinet canvases and is not affected.

## Resume
`CARGO_TARGET_DIR=... cargo test -p novo-app --lib rail`; QA scenario `qa/scenarios/730-tool-rail.qa`.
