# Heat shell (H1): handoff

State 2026-11-04: engine, 2D and 3D wiring and tests done; PR open from branch `claude/project-thread-v93a2s`. No app UI by this lane: the Shell section and the wall-temperature tint are lane H7 (design PR #1856, section 5 asks for exactly the fields below).

Code: `crates/novo-thermal/src/shell.rs` (all of it), `setup.rs` (g_wall, plate path, wall capacity), `report.rs::shell_report`, `crates/novo-cfd/src/shell_view.rs`, `solids/setup.rs::wall_conductances` and `plate_sheet`.

## Data contract (additive, stable face order)
Face order everywhere: `FRONT`(door)=0, `REAR`, `LEFT`, `RIGHT`, `TOP`, `BOTTOM` (`shell::FACE_NAMES`).
- Input: `Params::shell: Option<ShellModel>`; `Scenario::shell()` returns the one in use. `ShellModel { faces: [FaceSpec; 6], window: Option<Window>, plate_setback_mm, plate_emissivity, films: Films{h_in,h_out}, outside: Outside{sky_c,wind_m_s,sun_w_m2} }`. `FaceSpec { layers: Vec<Layer{material, thickness_mm}>, emissivity_in, emissivity_out, absorptance_solar }`. `Outside` is stored only (H3 uses it).
- Output: `ShellReport { faces: [FaceTemps; 6], plate_rear_c: Option<f64>, plate_setback_mm, capacity_per_face_j_per_k, capacity_j_per_k, caps: ShellCaps, status: ModelStatus }`. `FaceTemps { exposed_share, heat_w, u_w_m2k, inner_air_c, inner_c, outer_c, inner_min/max_c, outer_min/max_c, interfaces_c, window: Option<(inner,outer)>, outer_radiation_w, outer_convection_w }`. `faces[FRONT]` is the door.
- `ShellCaps { layers, emissivity, door_window, setback, outside }`: `outside` is false until H3.
- Heat per face sums to the loss minus openings and storage (2D balance; 3D within the solver's 3 %).
- Everything is `ModelStatus::Model`. Nothing is validated against a measurement.

## Traps
- IEC k is air to air: the default shell calibrates the films, never add films on top. `wall_k_scale` multiplies U.
- `exposed_share` is the IEC factor b: a covered face carries its flux through that share; the floor of a floor-standing cabinet has b=0 and its outer temperature equals the inside value.
- Outer radiation/convection split is a linearised model of the outer film, not a solved radiation problem (that is H2).
- Adding a field to `CfdSnapshot` needs a literal update in the app/heatview tests (done for `plate_rear_c`).
- Never name customer machines in the public repo.

Resume: `cargo test -p novo-thermal -p novo-cfd --test shell`.
