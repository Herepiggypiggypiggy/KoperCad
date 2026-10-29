# Visual redesign lane V5: palettes

Scope (docs/design/visual-redesign.md section 7, lane table V5).

Done in the first PR:
- Place palette (Q): category column, card grid with real symbol drawings at one scale, detail pane (large drawing with pins, tag prefix, standard, function, pins, aliases, catalog parts).
- Command palette (Ctrl K): icon per row (V0 presentation table), type chip, menu/rail/centre path chip, key cap, pinned tiles while empty.

Done in the follow-up PR: full-name tooltip on cut-off place palette cards; Ctrl K add-on chip "Add-on: name"; symbol thumbnails in the part pickers (Inspector, selection sheet, Cables drawer).

Next, in order:
1. Real part thumbnails (part CAD or maker image) when the parts importers deliver them; the symbol thumbnail is the fallback.
3. Category order and folder icons for a NFPA pack; user symbol folders.
4. If #1653 or other new commands land: add rows to `novo-theme/src/presentation.rs`.

Open questions: whether the detail pane should show catalog parts by symbol function only (current rule) or let a part name its symbol.
