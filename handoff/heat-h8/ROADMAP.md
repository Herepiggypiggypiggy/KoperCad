# Heat H8: internal fans and forced convection (roadmap)

Question (Tadjiro 2026-11-04): does forced convection from internal fans cool components? Yes; the model now shows it.

## Done (PR 1, engine, `novo-thermal`)
- `forced.rs`: flat plate (Blasius, mixed turbulent), turbulence lift (model), wake turbulence (model, plane-wake exponent), impinging round jet (Martin/Incropera, range-checked), free round jet speed field (axial-fan core 2 D, model), Elenbaas natural channel, fin efficiency, `Heatsink` film.
- `Params::circulation_fans` (`CirculationFan`, in `Params` so existing `Scenario` literals stay valid and H1 can add its `ShellModel` without a clash): in-plane fans are faces with the straight fan curve in the 2D pressure system; across-the-depth fans stir and impinge.
- `Params::heatsinks`, `Params::wake_turbulence` (default off).
- Result: `ThermalResult::circulation` (flow, pressure, share of free flow).
- Tests: `tests/circulation.rs`, `forced::tests`.

- UI (same PR, `novo-app/src/app/heat_inside_fans.rs`): "Fans inside" section in the 3D view's Vents and fans panel (Cabinet, Heat layer): add, position, direction, off-plate height, diameter, datasheet ends, installed %. Saved as structure property `heat.inside_fans`; `HeatSettings::apply` adds them to `Params::circulation_fans`. Text behind "?".

## Next
1. Draw the fan and its jet in the Cabinet canvas and the 3D view; Heat layer jet arrows (coordinate with H7). Drag to move.
2. 3D CFD PR (new branch from main after #1861 merges): 3D CFD (`novo-cfd`): internal fan as a momentum/pressure-jump patch on an interior face pair; device films use the resolved jet; compare with the 2D jet speed field.
3. Hot-spot air speed check against a fan case with measured/known axial-fan jet data (needs a datasheet-grade source; none in repo yet).
4. Heatsink data on parts (fins, gap) so `heatsinks` is filled from the catalog, not by hand.
5. Wake turbulence in the 3D films (from the resolved turbulence kinetic energy when a turbulence model runs).

## Open questions
- Jet core length of a cabinet axial fan (2 D assumed). Needs one measured case.
- Fan stall region (curve is a straight line; pressure above shut-off reverses the flow in the model).

## Real enclosure feed (heat coordinator 2026-11-04)
UI PR must take fan start position, plate, interior size and fan cut-outs from the chosen enclosure record (openings: position, size, free area), and label any default "assumed" next to the field, visible, not hover-only. Today `InsideFan::start` uses the plate centre of the shown enclosure and a 120 mm / 100 m3/h / 60 Pa planning fan: mark these "assumed" until a catalogue fan is chosen. Commit this note with the next PR (the branch is READY, no push now).
