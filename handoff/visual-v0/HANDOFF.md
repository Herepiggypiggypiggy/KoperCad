# Visual redesign lane V0: handoff

Updated 2026-10-28. Branch `claude/project-thread-n1sqyl`, based on main a2293c4.

## What exists
- `crates/novo-theme/src/drawn/` : `mod.rs` (Shape, Seg, Icon, Group, `prims`, `paint`, `show`, `stroke_width`), `set.rs` (GENERATED, 40 icons, `#[rustfmt::skip]`), `tests.rs`. Reached as `novo_theme::icons::custom::*` and `novo_theme::icons::paint`.
- `crates/novo-theme/src/presentation.rs` : `TABLE` (command id, `Glyph::Drawn|Phosphor`, short label <= 28), `of`, `glyph`, `short_label`.
- `crates/novo-workflow/src/homes.rs` : `Home`, `homes(id)`, `homeless()`, `census()`, `RELOCATED` (empty on purpose).
- `crates/novo-theme/src/drawn/runtime.rs`: runtime (add-on) icons. `crates/novo-app/src/app/addons_look.rs`: copies `Extensions` into the look tables.
- `crates/novo-workflow/src/presentation_tests.rs` : every command has a row, no stale row, short <= 28 and <= label, shorts unique within a menu level.
- New command `view.icon_gallery` (palette-only) opens `crates/novo-app/src/app/icon_gallery.rs`.
- QA scenario `qa/scenarios/720-icon-gallery.qa`.

## Add-ons (meets lane C0b, which landed first on main)
The add-on manager owns what add-ons contribute: `novo_addons::Extensions` (commands, panels, icon definitions, validated at install: label <= 28, <= 48 shapes, coordinates 0..24, names `<addon-id>.<name>`). V0 does NOT keep a second command registry (an earlier version of this branch had `novo-workflow::extension` and `commands::all()`; dropped to avoid two sources of truth). Palette rows come from `Sources.addons` as before.
What V0 provides for add-ons:
1. Painter for runtime icons: `novo_theme::drawn::register(IconSpec)` (same rules and engine as the built-in 40; `Icon::by_name` finds both; `clear()`, `unregister_prefix`, `registered()`). Data is leaked (a few bytes per icon per rescan); chosen over `Cow` shapes because it keeps `&'static Icon` and `Glyph: Copy` everywhere.
2. Presentation rows at runtime: `presentation::register_extension(id, icon_name, short)`, `register`, `clear`, `unregister_prefix`, `registered`; lookups (`of`, `glyph`, `short_label`) see static `TABLE` first, runtime rows second; an `addon.` id with no row gets `ADDON_ICON` (Phosphor puzzle piece). No 41st drawn icon.
3. Glue: `crates/novo-app/src/app/addons_look.rs::sync(&Extensions)` runs at the end of `refresh_addons`: replaces (never appends) the registered icons and command rows, so a disabled or removed add-on leaves nothing. Add-on arcs are degrees counter-clockwise on screen; converted to the painter's radians.
Not done, owned by later lanes: rail rows (`Extensions::rail_rows` into the V3a rail tables, max 6 per zone), app-menu footer (V2b), "Add-on: name" chip in Ctrl K (V5), keys, add-on home in the home invariant (static commands only; the manager validates add-on homes).

## Rules for later lanes
- A new command needs one line in `presentation.rs` (the test says so) and a Dutch line in `language.rs`.
- A lane that moves a command to a rail, centre or drawer toolbar adds it to `homes::RELOCATED` and removes the menu row; the invariant test keeps it from being lost.
- Regenerate icons: edit `docs/design/visual-redesign/tools/koper_icons.py`, then run `python3 docs/design/visual-redesign/tools/koper_icons.py docs/design/visual-redesign/icons` and `python3 docs/design/visual-redesign/tools/icons_to_rust.py > crates/novo-theme/src/drawn/set.rs`. A test compares shape counts with the SVGs.
- `palette::where_it_lives` still says "Keys" for a command with no menu row; V2c/V5 should use `homes::homes`.

## Traps
- Format of arcs: the SVG arcs are converted to centre form (angles in radians, clockwise on screen); the estop dome is an ellipse arc (rx 7.5, ry 6).
- The sensor icon's outer wave reaches x = 22.9 (live area is 2 to 22, stroke still inside 24); named in the test.
- `horizontal_wrapped` does not wrap frames in egui; the gallery lays rows out by hand.
- Not verified: Windows fonts, 125 and 150 percent scaling, real GPU colour. At 16 pt the cable-section icon is the weakest (four cores read as a blob); everything else reads.
