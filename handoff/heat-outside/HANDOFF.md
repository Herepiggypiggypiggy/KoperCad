# Heat H3 (outside convection, mounting, sun): handoff

State 2026-11-04: `novo-thermal/src/outside.rs` merged-pending: pure correlations, no solver wiring (waits for H1 shell contract).
Contents: air properties, Churchill-Chu vertical, McAdams/Lloyd-Moran horizontal, flat-plate forced, Churchill mixed blend, linearised radiation, Swinbank sky, Elenbaas gap (wall behind, row neighbour), sun absorption, `solve_outer_surface` (bisection balance) and `overall_u`.
Honest limits: MODEL. Tests are hand calculations, limits and energy balance. Elenbaas wide-gap limit sits about 25% under Churchill-Chu (within correlation scatter). Wind applied as flat-plate along the face; no building-wall wind pressure effects. Sun irradiance per face is an input (no solar geometry yet).
Next: when H1 contract lands, map `Outside {ambient, sky, wind, sun, mounting}` to per-face `OuterFace`, replace IEC b-factors only when a test shows the overall U within the IEC band (never add films on top of IEC k). UI belongs to H7 (Cabinet properties, behind "?").
Traps: IEC k is air-to-air; do not double count radiation with the 2D film.
