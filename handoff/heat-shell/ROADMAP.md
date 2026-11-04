# Heat shell (lane H1): roadmap

Scope: the enclosure walls, door and plate as real layers (material, thickness, heat capacity), inner and outer surface temperatures, replacing the single IEC 60890 per-face number. Owner of the data contract that H2, H3, H5, H7 build on.

## Done (PR on branch claude/project-thread-v93a2s)
- `novo-thermal::shell`: `ShellModel`, `Layer`, `LayerMaterial`, `Finish`, `Window`, `Films`, `Outside`, `ShellReport`, `WallStack` (transient 1D wall).
- Default shell calibrated to IEC 60890: results unchanged (test: every material and mounting, 1e-9).
- 2D solver (`Setup`) and 3D solver (`wall_conductances`, plate sheet) use the shell; `Params::shell: Option<ShellModel>` (None = default shell of the enclosure).
- Result: `ThermalResult::shell` (2D), `novo_cfd::shell_view::shell_report(scn, snap)` (3D), `CfdSnapshot::plate_rear_c`.
- Tests: hand-calculated series resistance, lumped cooling, semi-infinite slab (erfc), steady profile, 2D and 3D insulation/window/setback cases.

## Next
1. Door gasket / edge bridge (linear conductance W/mK x perimeter), door-mounted devices.
2. Lateral conduction in the sheet (corner and edge effects) if H6 shows a gap.
3. Hand the wall temperature field to H2 as wall nodes (radiation) and to H5 (WallStack in the transient 2D/3D march).
4. Persist the shell on the enclosure in the project (needs a novo-core change by WS-02; until then the app uses the default shell).

## Open questions
- Inner/outer film split is an assumption (`INNER_FILM_SHARE` 0.5). A measured cabinet would fix it.
- Setback path (air gap + rear wall) is a model; no exact case yet.
