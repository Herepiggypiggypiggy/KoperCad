# Heat H8: internal fans and forced convection (roadmap)

Question (Tadjiro 2026-11-04): does forced convection from internal fans cool components? Yes; the model now shows it.

## Done (PR 1, engine, `novo-thermal`)
- `forced.rs`: flat plate (Blasius, mixed turbulent), turbulence lift (model), wake turbulence (model, plane-wake exponent), impinging round jet (Martin/Incropera, range-checked), free round jet speed field (axial-fan core 2 D, model), Elenbaas natural channel, fin efficiency, `Heatsink` film.
- `Params::circulation_fans` (`CirculationFan`, in `Params` so existing `Scenario` literals stay valid and H1 can add its `ShellModel` without a clash): in-plane fans are faces with the straight fan curve in the 2D pressure system; across-the-depth fans stir and impinge.
- `Params::heatsinks`, `Params::wake_turbulence` (default off).
- Result: `ThermalResult::circulation` (flow, pressure, share of free flow).
- Tests: `tests/circulation.rs`, `forced::tests`.

## Next
1. PR 2: UI. "Fans inside" section in the 3D view's Vents and fans panel (add, position, direction, datasheet ends, installed factor, height off the plate); stored as a structure property like the openings; Heat layer shows the jet arrows (coordinate with H7).
2. PR 3: 3D CFD (`novo-cfd`): internal fan as a momentum/pressure-jump patch on an interior face pair; device films use the resolved jet; compare with the 2D jet speed field.
3. Hot-spot air speed check against a fan case with measured/known axial-fan jet data (needs a datasheet-grade source; none in repo yet).
4. Heatsink data on parts (fins, gap) so `heatsinks` is filled from the catalog, not by hand.
5. Wake turbulence in the 3D films (from the resolved turbulence kinetic energy when a turbulence model runs).

## Open questions
- Jet core length of a cabinet axial fan (2 D assumed). Needs one measured case.
- Fan stall region (curve is a straight line; pressure above shut-off reverses the flow in the model).
