# Heat model and cooling: roadmap

Scope: plan item R3 of `docs/PRO-PANEL-GAP-ANALYSIS.md`. Total loss, effective
cooling surface, interior temperature rise, required cooling power, a loss-density
hot-spot map on the Cabinet canvas. Design and formulas: `docs/design/thermal.md`.
Engine: `novo-layout::thermal`. Sibling: Enclosure sizing helper (owns size
recommendations). Needs `power_loss_w` from the Part power loss and clearances thread.

## Done
- Design doc with formulas and the standard followed (IEC 60890 effective area, heat balance).
- `novo-layout::thermal`: enclosure, effective area, balance, heat map, legend, hot devices, model bridge.

## Next, in order
1. Heat overlay scene and legend in `novo-canvas` (colour bands plus marks, not colour alone).
2. App: "Heat" toggle on the Cabinet canvas, result strip with the planning note.
3. Enclosure inputs in the inspector (size, installation, material, ambient, limit).
4. Problem in `novo-checks`: needs about N W of cooling or V m3/h.
5. IEC 60890 curves if the tables become available.

## Open questions
- Final part key names from the Part power loss thread (`power_loss_w`, `load_factor` assumed).
- Enclosure dimensions: kept as `Structure` properties until the enclosure thread adds fields.
