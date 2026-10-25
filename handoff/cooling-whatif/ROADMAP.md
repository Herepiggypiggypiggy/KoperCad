# Cooling what-if wizard: roadmap

Scope (from audit/creative/round34.md idea 2): one wizard that asks for the conditions (hottest day, duty), tries the fixes against each other in the same model, ranks them, previews the picked one in 3D with a before and after table, and applies it. Honest labels: heat/CFD are models, ENCLOSURE_VALIDATED stays false.

## Done (branch claude/project-thread-qk7tim)
- Engine `novo-thermal/src/whatif.rs`: `Conditions` (ambient, duty), `run()` tries layout (cooler layout), vent/fan pairs (best passive + best powered), layout + openings when neither alone clears every limit, bigger enclosure (model only). Ranked by devices over limit, tightest margin in half-kelvin steps, then effort. Tests `crates/novo-thermal/tests/whatif.rs`.
- App `novo-app/src/app/heat_whatif.rs`: window with MODEL badge, conditions, ranked cards, before/after table, copy-as-text, Apply. 3D preview in `view3d.rs` (green boxes plus a line for moves, openings drawn as markers). Tests `heat_whatif_tests.rs`.
- Entries: Cabinet and 3D Heat strip button "Cooling what-if", plate right-click menu, palette "Cooling what-if wizard" (`heat.cooling_whatif`).
- QA scenarios 400-402.

## Next
- Done: 3D vs 2D comparison of the ranking (#1483) and of fan gains (fan sweep PR): ranking holds, fan gain is read low, no single calibration factor.
- Done: fan jet speed term in the 2D engine (setup.rs `JET_SPEED_COEFFICIENT`), frozen-reference test `novo-thermal/tests/fan_gain.rs`, `FAN_NOTE` reworded.
- Next: the 600x600x400 box over-predicts (3D/2D 0.69 to 0.92) and the tall box under-predicts (up to 1.28): find what separates them (device distance from the fan, depth) before touching the coefficient; re-run `validation_fan_gain.rs --ignored` after, and add a fourth box to hold out. A measured cabinet would replace the 3D solver as the reference.
- Cooling unit / filter fan product pick from Cooling selection as a fix kind (needs the wall balance, not the 2D model: different number, label it).
- Duty by device (per-device load factor) instead of one global duty.
- Print the before/after table into the bench packet.
- 3D CFD check of the picked fix from the wizard (today: apply, then Simulate).

## Open questions
- Whether the "bigger enclosure" step should use the next catalogue size instead of x1.25 rounded to 100 mm.

## After the position sweep
- The honest 2D answer to "beside the jet" needs the 2D model to know the jet line (fan to vent) and a lateral decay; one fit on 48 cases did not generalise. A better reference than the 3D reduced model (a measured cabinet) would settle which side is right before fitting again.
- A per-device warning in the what-if ("device T1 is beside the fan-to-vent line") is cheap and needs no new physics: the line and the device positions are known.
