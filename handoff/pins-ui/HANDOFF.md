# Handoff: PINS-UI

Branch `claude/project-thread-uoh732`. Reads on top of handoff/part-pins (engine in novo-interop part3d/pins.rs, merged).

- `crates/novo-routing/src/model.rs` `anchor_of`: uses `novo_parts::terminal_position` of the pin's name from the device's parts. Schema frame (y up, back-left-bottom) to plate (y down); turned devices go clockwise seen from the door; z is the terminal height. Pins the part does not know keep the footprint-edge anchor.
- `CatalogPart::to_model` writes the positions (`pin.<name>.x_mm` ...).
- `crates/novo-app/src/app/parts_pins.rs`: the block in the part detail (Parts window > select part > Connection points). Scan runs on a thread. Save upserts into the user catalog. Estimated points are saved only with the checkbox.
- Tests: novo-routing/tests/pin_positions.rs, novo-parts/tests/pin_positions_model.rs, app/parts_pins_tests.rs.
- No real maker STEP file has been tried (maker sites are blocked from the cloud): run it on a real part on the PC.
