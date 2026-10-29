# Visual redesign lane V5: handoff

Branch `claude/project-thread-cfh1v9`, from main 8c9c97e (V0 merged).

## What exists
- `crates/novo-app/src/place_palette.rs`: symbol artwork painter (`paint_symbol`, `pieces`, `common_scale`), categories (`Category`, `category_lines`, `category_icon`), `rows_for` (what a category shows), widgets (`category_column`, `card_grid`, `detail_pane`), command palette row pieces (`kind_glyph`, `paint_chip`, `paint_key_cap`).
- `crates/novo-app/src/command_field.rs`: `show` now draws the place palette (Mode::Place, 880 wide) and the restyled Ctrl K rows; pinned tiles from recent commands then `PINNED_DEFAULT`.
- The search itself (`novo-workflow::palette`) is unchanged; `enter`, `rows`, `type_text` and lessons behave as before.
- QA scenarios `qa/scenarios/730-place-palette.qa`, `731-command-palette-icons.qa`.

## Keys
Place palette: arrows move by card (Up/Down a row of cards; Left/Right when the field is empty, otherwise Alt+Left/Right), Tab/Shift+Tab walk the categories, Enter places, Esc closes. Digits 1 to 9 still pick favourites.

## Traps
- Not verified on the cloud box: screenshots (no Xvfb/openbox/vulkan packages); run `scripts/qa.sh --only palette` on a box that has them. Nothing was seen on a real GPU or Windows.
- Common scale: the fit scale 80 percent of the shown symbols can use; the largest fifth is shrunk to fit on its own.
- Catalog parts in the detail pane are matched by letter code and part function; the starter catalog may list none for a symbol.
