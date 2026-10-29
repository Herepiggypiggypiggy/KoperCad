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

## V3b and V3c: 3D and Cabinet rails (2026-10-29, branch claude/project-thread-of28bw)
- `rail_3d.rs` and `rail_cabinet.rs` hold the row tables (`TOOLS`, `ASSISTS`, `PANELS`), `view3d_rail()` / `cabinet_rail()` and the click handlers. Both draw through `rail_draw` (rail_schematic.rs), so the expand choice is the one `rail_expanded` setting. `look()` chains the three `local()` tables.
- A row is a command id; a click queues it, so the rail owns no state. 3D `active` is read from the option registry (`ALL` spec whose `command` equals the row) or from the card that owns it. Cabinet `active` is read from `cabinet.rails.open`, `routes`, `rules.editor_open`, `rules.cut_open`, `emc.on`, `heat.on`, `table`.
- 3D flyouts: Align (six align rows, caption Spread over two), Hide (hide, isolate, show all). Their entries only open the card (`rail::open`).
- The 3D toolbar (with the sim status chip) and the Cabinet bar stay for now: they hold Layers, the door slider, the Plate/Table switch, Accept/Cancel of a preview and the fold-into-More logic. They start right of the rail because the rail is a real left panel. `view3d_controls::below` now also follows the drawn toolbar height, since the 48 pt of the rail can wrap the toolbar onto another row.
- Not built, on purpose: the 3D side panel (Side panel, Layers rows; Position and Statistics stand in the panels zone), Select on the Cabinet rail (no tool in hand), the Cabinet Inspector row and a Heat flyout. Trim the toolbar/bar duplicates once the lessons are re-anchored (V7).
- QA: `qa/scenarios/770-cabinet-rail.qa`, `771-3d-rail.qa`. Tests: `cargo test -p novo-app --lib rail_workspaces`.

## V3d: Reports and Commissioning rails
- `rail_reports.rs`: rows of both rails (`REPORTS_*`, `COMMISSIONING_*`), `local()` for `rail.*` rows, `reports_rail_show` / `commissioning_rail_show`, `workspace_rail_click`. Dispatch is in `app.rs` next to the other rails; tests in `rail_reports_tests.rs`.
- Reports: Add as drawing page, Report designer, Labels | Bench output, Build mode | Export (flyout: Excel, report PDF, BOM, PDF), Print preview.
- Commissioning: the four panel tabs (Checks, Punch, Redlines, Test) | Next open (moves only, never decides; Mark done stays the Space tick) | Export commissioning, Export punch list. The design's Photo and Note tools do not exist as tools.
