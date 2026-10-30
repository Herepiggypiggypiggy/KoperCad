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
- Budget: `menu_rows_stay_inside_the_budget` has a ceiling per menu (File 88, Edit 30, Draw 39, View 65, Check 11, Reports 30, Simulate 24, Help 19 = 306 after main added New from template and one Edit row). The ceilings are exact: a PR that adds a menu row raises its menu's ceiling by one and says why the row needs a menu; moving rows lowers it. The design target is about 152.
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

## V4a: the Export centre (2026-10-29)
- `novo-workflow/src/centres.rs` lists the formats the centre shows, group by group (Drawings, Print, Reports, Workshop and bench, PLC, Exchange), with one sentence for each (`describe`). Every `export.*` command is in exactly one group (a test keeps it so). A new export command goes into a group there, or the test names it.
- `novo-app/src/app/centre.rs` (was `export_centre.rs`) is the window for both centres (`modal_fill`, 1120 by 640, `dialog_width::CENTRE`): search on top, tiles by group on the left, the picked format on the right, Export button at the foot. A tile click picks, a double click runs, Enter runs the picked one; the search keeps the keyboard so typing keeps narrowing. Each tile carries the lesson anchor `cmd.<id>`; while the centre is shut the anchor falls back to `cmd.file.export_centre` and then to the File menu.
- `file.export_centre` ("Export...") is the one File row. The 44 other export rows left File and Reports (File 88 to 58, Reports 30 to 17). `export.print_preview` keeps its File row as the print entry and is also a tile. All stay in the command field.
- Lessons that walked File > Export submenus now say "open Export... and pick the format"; the manual lines too. Scenarios 09, 184 and 723 changed for it; 726 is new.
- Budget ceilings now: File 60, Edit 30, Draw 39, View 65, Check 11, Reports 17, Simulate 25, Help 19 = 266 (Simulate 25 because `sim.hoist3d` landed after V2c and left main's own budget test red).

## V4c: the Import centre (2026-10-29)
- `centres::Centre` is `Export` or `Import`; `IMPORT_SECTIONS` groups the 14 commands of File > Import and File > Import parts (Drawings and backgrounds, Projects, Parts, Lists and tags). The window is the same (`centre.rs`, `CentreWindow { kind, .. }`, `wb.centre`); the footer button says Import, or Remove, Choose, Get script for the three that are not imports.
- `file.import_centre` ("Import...") is the one File row; File 60 to 47, total 266 to 253. Palette and app-menu search show `File > Import` for the tiles; anchors fall back to `cmd.file.import_centre`.
- Tile minimum width 250 and the window 1120 wide: titles like "Export plate drawing as DXF" were cut at 1366x768.
- QA scenarios that clicked `File > Import parts > ...` (21-parts-*, 60, 61, 639, 1700 and the `import_table` step) go through the centre: type the query, click the tile, press the Import button.

## V4d: drawer toolbars (2026-10-29)
- `homes::DRAWER_TOOLBARS` lists, by the command that opens the drawer, the 11 `terminal.*` strip edits (Terminals drawer toolbar and row menu: `terminal_edit::EDIT_COMMANDS`, a test keeps the two equal) and the 4 `cable.*` rows (Cables drawer: name field, type picker, Delete cable, Restore declined). Their Draw rows (submenus Terminal strips and Cables) are gone: Draw 39 to 24, total 253 to 238. `Home::DrawerToolbar` is set for them; `menus::moved_out` knows them.
- Lessons: `cmd.terminal.*` anchors are the toolbar buttons (`strip_toolbar` registers a spot for each); with the drawer shut they fall back to the rail button `cmd.view.terminals_drawer`. The palette and the app menu search say `Terminals drawer` or `Cables drawer` as the path.
- QA scenario 728 (Draw menu without the submenus, the toolbar in the drawer, the palette path).

## V4e: panel homes (2026-10-29)
- New `Home::Panel(name)` (`HomeKind::Panel`): a button or context menu of a panel. `homes::PANEL_BUTTONS` lists the 6 `page.*` commands (Pages list: the + New button and the right-click menu of a page) and the 4 `learn.*` commands (lesson card: Back, Next, Start over, Close). Their File > Pages and Help > Lesson rows are gone: File 47 to 41, Help 19 to 15 (the design target), total 253 to 243.
- The palette and the app menu search name the panel as the place; lesson anchors `cmd.page.*` fall back to the Pages list (`outline.pages`); the tags-and-pages lesson says "Right-click the page in the Pages list".
- QA scenario 729.

## V4 what is left
- **Options and preview on the right of the centre** (design section 6): the right side now shows the sentence, the key and the reason a format is grey, not the options of the format or a preview. Formats with options (PDF options, print sets, DXF) still open their own window after Export.
- **Library rows** (File > Library, Macros) are still menus.
- **Project and Pages** moves, as in the V2c gaps above; the remaining Draw rows (Terminals submenu, PLC, Macros) and View (65 rows).
- The Reports rail's Export flyout (4 rows) still lists a few formats; it should open the centre once the rail has been used for a while.

## V4f: Start centre (2026-10-30)
- `centres::Centre::Start` (command `file.start_centre`, File > Templates and examples...): the 3 `file.new_*` templates and the 6 example commands as tiles (Create / Open button). The File > New from template and Examples submenus and the EMC example row are gone: File 41 to 34, total 228 to 221 (after #1763). `file.example` keeps its row in File > Open (`centres::ALSO_IN_MENU`) and is also a tile.
- Lessons: `cmd.file.new_*` and `cmd.file.example_*` fall back to `cmd.file.start_centre`, then File. Palette and app menu search say `File > Templates and examples`.
- QA scenario 7370. Not done: preview of the template or example in the detail pane; options and preview pane for the other centres.

## V4g: Project settings centre (2026-10-30)
- `centres::Centre::Project` (command `file.project_centre`, File > Project settings...): Drawing standard, Rules, Report languages, Project options, Untie from options and Generate pages as tiles. The File > Project submenu (8 rows) and Report languages leave File: File 34 to 26, total 221 to 213.
- `project.variant`, `project.option` and `project.tie` take a name in the command field and open Project options without one: they are palette-only (`menus::PALETTE_ONLY`, with the reason). Lessons that name them fall back to the opener.
- QA scenario 7371.

## V4h: Numbering centre (2026-10-30)
- `centres::Centre::Numbering` (command `edit.numbering_centre`, Edit > Numbering...): Number wires, Renumber wires, Check wire numbers, Wire numbering, Set and Unlock wire number ("Wires") and Renumber devices, Number interruption points, Renumber I/O ("Devices and points"). The Edit > Wire numbers and Edit > Renumber submenus (9 rows) are gone: Edit 30 to 22, total 213 to 205. Tiles that need a selection say why they are grey.
- Lessons that name those commands fall back to the opener, then Edit. QA scenario 7372. Next: Parts rows (Edit > Parts, File > Library), then the Draw leftovers.

## V4j: terminal rows leave Draw (2026-10-30)
- The seven Draw > Terminals rows (Connect and Disconnect terminals, Toggle PE terminal, Reset PE override, strip number Up, Down, Lowest free) act on the terminals selected on the page. Their home is the right-click menu of a selected terminal, which already carried each one: `homes::PANEL_BUTTONS` entry "Terminal right-click menu". Draw 24 to 17, total 205 to 198 (after #1772). Up and Down keep their keys; every row is still in Ctrl+K (its path reads "Terminal right-click menu").
- Test `the_terminal_right_click_menu_carries_every_terminal_row_that_left_draw` reads the editor's menu source, so a row dropped from the menu fails it. QA scenarios 7374 (1366x768) and 7375 (1920x1080) walk the terminal-strips lesson to two selected terminals and open the menu.
