# Visual redesign lane V5: palettes

Scope (docs/design/visual-redesign.md section 7, lane table V5).

Done in the first PR:
- Place palette (Q): category column, card grid with real symbol drawings at one scale, detail pane (large drawing with pins, tag prefix, standard, function, pins, aliases, catalog parts).
- Command palette (Ctrl K): icon per row (V0 presentation table), type chip, menu/rail/centre path chip, key cap, pinned tiles while empty.

Next, in order:
1. Part thumbnails in part pickers (needs a thumbnail source: part CAD or symbol).
2. Add-on chip text "Add-on: name" in Ctrl K (type chip says Add-on today).
3. Category order and folder icons for a NFPA pack; user symbol folders.
4. If #1653 or other new commands land: add rows to `novo-theme/src/presentation.rs`.

Open questions: whether the detail pane should show catalog parts by symbol function only (current rule) or let a part name its symbol.
