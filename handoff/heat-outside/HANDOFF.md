# Heat H3 (outside convection, mounting, sun): handoff

State 2026-11-04: correlations merged (#1857). Wiring PR: physical outer film in the 2D solver on top of the H1 shell.

## What exists
- `novo_thermal::outside`: correlations + `solve_outer_surface` + `resolve(&Scenario)`.
- Input: `ShellModel::outside` = `Outside { film: OutsideFilm::{Iec (default), Physical}, sky_c, wind_m_s, sun_w_m2, sun_share[6], gaps_mm[6], h_out_face, sun_offset_k }`. Face order door, rear, left, right, top, bottom.
- `Physical`: film from the outer surface temperature per face (natural + wind, radiation to room/sky), mounting by gap to wall or neighbour (`gaps_mm`; a face the installation marks covered gets ASSUMED_COVER_GAP_MM = 20 mm, flagged assumed), sun as sol-air rise of the air a face sees (`sun_offset_k`, solver adds it to ambient per face). IEC `b` factors are replaced by 1 (floor-standing bottom 0) so mounting is not counted twice.
- `resolve` iterates the steady solve (max 12, damped) and freezes `h_out_face` and `sun_offset_k` in the shell; `simulate_steady_with` and `simulate_transient` call it. Already resolved scenarios return unchanged. `Iec` returns the scenario unchanged (results identical to before).
- Tests: `crates/novo-thermal/tests/outside.rs` (wind, sun, paint, gap, row, assumed gap, balance, idempotence).

## Honest limits (all MODEL)
- Physical total sits within about 0.6x to 1.6x of the IEC wall (test band); not validated against a measurement. H6 decides.
- Sun offset is a linear sol-air shift at the converged film; irradiance per face is an input (no solar geometry).
- Plate-to-rear path with setback still uses H1's path; rear face mean uses wall + plate cells.
- 3D (novo-cfd) reads `Scenario::shell()` so a RESOLVED scenario gives it the films, but the sun offset is NOT applied in 3D yet. Feed 3D with `outside::resolve(&scn)`.
- Bottom of a floor-standing cabinet stays adiabatic; plinth/ground contact not modelled.

## UI (H7)
Cabinet properties, Shell section: film Iec/Physical, mounting gaps per side, wind, sun and share, sky; labels "model" / "assumed" behind "?". Not in this PR.

## Next
Solar geometry (date, latitude, orientation); sun offset in 3D; plinth; validation with H6.
