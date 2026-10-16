# Thermal simulation: visuals: roadmap

Scope: how thermal and airflow results are drawn and where the user finds them (Heat view in the Cabinet view, later the 3D face). Design: `docs/design/thermal-visuals.md`. Physics engine is the "Thermal simulation: physics engine" thread; the #406 heat layer is "Heat model and cooling".

## Done
- Design doc and three SVG mockups (this PR).

## Next, in order
1. Result contract agreed with the physics thread (section 8 of the doc).
2. `novo_theme::heat`: ramp, lookup table, diverging ramp, tests.
3. `novo-heatview` crate (coordinator registers it): sampling, contours, hot spots, streamlines, particles, flux, compare, synthetic generator.
4. Cabinet Heat view: map, contours, legend, badges, callouts, palette entries.
5. Airflow, flux, animate toggle.
6. Time slider and compare.
7. 3D face after #481.

## Open questions
- Solver resolution; legend marker source (lowest limit vs sizing limit).
