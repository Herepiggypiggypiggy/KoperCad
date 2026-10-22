# Wire heat in ducts: handoff

Branch `claude/project-thread-ianqu6` (repo herepiggypiggypiggy/kopercad). PR and head SHA: see PROGRESS.md.

## Design
- Pure physics in `crates/novo-thermal/src/wires.rs` (`DuctWire::loss_w`, `loss_by_duct`). Copper at the insulation maximum (70 C PVC, 90 C XLPE): upper planning value.
- `Obstacle::loss_w` carries the watts. 2D: `Setup::sources` (air, per cell under the duct). 3D: `novo-cfd/src/solids/setup.rs` puts `loss / box volume` into `heat_w_m3` of the duct cells.
- `BridgeOptions::duct_loss_w` (rail id to watts) is the only way in; `BridgeReport::duct_loss_w` echoes it.
- `crates/novo-app/src/app/heat_wires.rs` finds the wires: single-line links give current (stated or calculated from loads below) and section; `novo_routing::model::route_project` gives the path; `length_in_ducts` clips the path to each duct's centre line. Conductors: 3 for three-phase, 2 otherwise (same convention as the drop calculation). Other conductors of a run are assumed to lie like the listed one.
- Diversity: none unless the project states `heat.wire_current_factor` (0..1, scales the current).
- Notes go to `Solution::wire_notes` and show in the Temperature panel under the warnings (not as warnings).

## Resume
```
export CARGO_TARGET_DIR=/home/claude/target
cargo test -p novo-thermal -p novo-cfd --test duct_wire_heat --test duct_wires
cargo test -p novo-app heat_wires
scripts/qa.sh --only "wire" --out /mnt/project-files/qa/heat-wire-ducts/
```
Fixture rewrite: `cargo test -p novo-app write_wire_heat_fixture -- --ignored`.

## Traps
- The example project's wires have no cross-section, so the example shows only the "no cross-section" note; use the fixture.
- Control wires have no run current, so they are listed as "carry no current" and add nothing.
