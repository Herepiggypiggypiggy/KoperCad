# Cooling what-if wizard: roadmap

Scope (from audit/creative/round34.md idea 2): one wizard that asks for the conditions (hottest day, duty), tries the fixes against each other in the same model, ranks them, previews the picked one in 3D with a before and after table, and applies it. Honest labels: heat/CFD are models, ENCLOSURE_VALIDATED stays false.

## Done (branch claude/project-thread-qk7tim)
- Engine `novo-thermal/src/whatif.rs`: `Conditions` (ambient, duty), `run()` tries layout (cooler layout), vent/fan pairs (best passive + best powered), layout + openings when neither alone clears every limit, bigger enclosure (model only). Ranked by devices over limit, tightest margin in half-kelvin steps, then effort. Tests `crates/novo-thermal/tests/whatif.rs`.
- App `novo-app/src/app/heat_whatif.rs`: window with MODEL badge, conditions, ranked cards, before/after table, copy-as-text, Apply. 3D preview in `view3d.rs` (green boxes plus a line for moves, openings drawn as markers). Tests `heat_whatif_tests.rs`.
- Entries: Cabinet and 3D Heat strip button "Cooling what-if", plate right-click menu, palette "Cooling what-if wizard" (`heat.cooling_whatif`).
- QA scenarios 400-402.

## Next
- Validate the 2D model against a known case (analytic or published enclosure result) and say what it shows; ENCLOSURE_VALIDATED stays false until measured.
- Cooling unit / filter fan product pick from Cooling selection as a fix kind (needs the wall balance, not the 2D model: different number, label it).
- Duty by device (per-device load factor) instead of one global duty.
- Print the before/after table into the bench packet.
- 3D CFD check of the picked fix from the wizard (today: apply, then Simulate).

## Open questions
- Whether the "bigger enclosure" step should use the next catalogue size instead of x1.25 rounded to 100 mm.
