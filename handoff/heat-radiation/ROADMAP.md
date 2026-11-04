# Heat H2 radiation: roadmap

Scope (coordinator, 2026-11-04): radiation at every fidelity level with computed view factors: wall to wall, device to wall, wall to ambient, emissivity per surface. Parent: handoff/heat-research/ROADMAP.md.

## Done
- Step 1: `novo-thermal/src/radiation.rs`, pure geometry and radiosity (no solver wiring). Exact element-to-polygon view factor (Lambert), Gauss quadrature over the emitter, closed-form references (parallel, perpendicular rectangles), `Enclosure` with reciprocity and summation enforced, grey diffuse radiosity with temperature, power or adiabatic boundaries, two-surface reference.

## Next, in order
1. H1 shell PR #1865 is up (contract in heat-research/ROADMAP.md). Wiring waits until it is on main (no stacked PRs).
2. DONE: occlusion by blocker boxes (`view_factor_blocked`, `Enclosure::with_blockers`); compare with 3D ray-traced factors still open.
3. 2D model (`thermal.rs`): replace `Params::radiation_view_factor` (0.5) by a per-device exchange factor from `radiation.rs` (device to plate, walls, door), using per-surface emissivity. The favourable/unfavourable band then comes from emissivity ranges, not a view factor guess.
4. CFD: `RadMode::Auto` on at every level (patch count scaled to level), wall nodes from H1, outer wall to ambient/sky through H3's `outside.rs`.
5. UI entry (with H7): Heat layer radiation split, emissivity per surface in Cabinet properties, text behind "?".

## Honesty
Grey diffuse, no gas absorption, emissivities are planning values, factors exact only without obstacles. Label "model" until compared with a measurement.
