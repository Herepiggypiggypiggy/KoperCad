# Visual redesign lane V2: handoff

## What exists
- `crates/novo-app/src/app/app_menu.rs`: `mark()` (the button, a caret, `Popup::menu`), `app_menu_ui` (search, area list, rows, footer), `app_menu_keys` (arrows, Enter, Alt+letter), `area_entries` and `search_entries` (the data, tested without a window), `AppMenu` state in `Workbench::app_menu`.
- `Settings::menu_names: Option<bool>`, `show_menu_names()`, `MENU_NAMES_DEFAULT` (true for now). `NovoApp::menu_names_shown()` is wide window and setting; the top bar draws the menu words only then.
- `menu_bar.rs::hint_for` (tooltip sentences of export commands) is shared by the bar rows and the app menu rows.
- Tests: `app_menu_tests.rs` (every bar command is in the app menu, groups, search, F10 with names hidden and shown, Enter, Alt+letter, arrows, both themes at both sizes). QA scenario `qa/scenarios/721-app-menu.qa`.

## How a row looks
Icon (drawn or Phosphor, `presentation::glyph`), short label (`presentation::short_label`), key cap right; the tooltip is the full label, a hint sentence, and the reason when the row is grey. Submenus are folded groups with their count; a search shows every match with its menu path.

## Rules for the next lanes
- A new command needs a presentation row (V0) and appears in the app menu by itself (it reads `menus::drawn`).
- Lesson anchors: with the menu words hidden, `menu.<Area>` anchors sit on the area rows of the open app menu; `menu.project` stays on the mark. With the words shown nothing changed.
- V2c trims the registry; the app menu follows without a code change.
- V3 (rail, other thread) and V5 (palettes, other thread) do not touch `app_menu.rs`.

## Not verified
Windows fonts, 125 and 150 percent scaling, real GPU colour; a real screen reader (the painted rows carry a `WidgetInfo` label, so the accessibility tree and the QA `click_text` see them, but no screen reader was run).

## V2c: where the commands live now (2026-10-29)
- `menus::MOVED` (novo-workflow) lists the commands whose menu row left because a rail draws them; `menus::place` returns None for them and the app menu, the menu bar and the palette follow. They stay in the command palette. `homes::RELOCATED` names the rail as home; the test `a_command_moved_out_of_the_menus_has_a_home_elsewhere` and the app test `the_relocated_table_matches_what_the_rails_draw` keep both honest.
- A rail row that took over a menu row carries the lesson anchor `cmd.<id>` (`learn_labels::moved_anchor`), so lessons keep pointing at it.
- Budget: `menu_rows_stay_inside_the_budget` has a ceiling per menu (File 87, Edit 29, Draw 39, View 65, Check 11, Reports 30, Simulate 24, Help 19 = 304). Lower the ceilings when rows move; never raise them. The design target is about 152.
- Label policy: menu rows use the short label of the presentation table (28 characters at most, tested by the V0 presentation tests).

## V2c: what is left (gaps)
- **3D side panel** (`visual-redesign-3d.md` section 5) is not built: Heat, EMC, Position and Statistics are rail panel rows that switch an overlay; when the panel lands those rows open it and the entries are listed there.
- **Old toolbar buttons the rails duplicate**: the 3D canvas toolbar (`view3d_controls::toolbar`) and the Cabinet bar still draw their own buttons for Measure, Section, Hide, Align and the Cabinet tools (Rails, Rules, Cut list). They stay until the rails have been used for a while; removing them is a follow-up lane, not a registry change.
- **Import and Export centres (V4)** do not exist, so the File rows for open/import/export formats (87 rows) and the Reports exports (30) stay. File is the biggest menu and will fall most when V4 lands.
- **Drawer toolbars**: Draw > Terminals and Cables, and the strip and cable drawer commands, keep their menu rows until the drawers carry their own toolbars.
- **Help > Lesson** rows wait for the lesson card; **Project settings and Pages panel** moves wait for those panels.
- **Edit duplicates** (Undo/Redo also on the rail and in the top bar) stay in Edit: the menu is the place people look first and it shows the key.
- V7 flips `MENU_NAMES_DEFAULT` and the lessons/QA scenarios that click menu words.
- **Label policy in the classic menu bar**: the bar (menu words shown) still draws the full command labels (for example "Show or hide the wires in 3D", "EMC net classes on the schematic: each wire in the colour of its cable class (indicative)"). The short labels (28 characters) are used by the app menu, rails and palette. Moving the bar to short labels changes what lessons and QA scenarios click; that goes with V7 (menu words hidden by default).
