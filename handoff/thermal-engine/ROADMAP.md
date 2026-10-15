# Thermal engine (novo-thermal) roadmap

Thread: "Thermal simulation: physics engine". Design: `docs/design/thermal-simulation.md`. PR: #505.

Done
- Design note and input/result types (`model.rs`, `result.rs`).
- Coupled air, plate and device solver; reduced-order airflow (Darcy-Forchheimer
  with buoyancy, vents, fans, obstacles); IEC 60890 wall coefficients shared with sizing.
- Steady, transient, uncertainty band, hot spots, advice tried in the model.
- Physics and advice tests, demo example and picture.

Next
- Visuals thread draws `ThermalResult` (Cabinet view, Heat layer, "Simulate", palette `cabinet.thermal.simulate`).
- Bridge from project (placements, part loss and temperature limits) to `Scenario`, owned by "Heat model and cooling" / UI threads.
- Part properties for limits: `max_ambient_c`, `max_surface_c`, device height (ask Parts).
- True 3D depth profile (door versus plate) and a second slice (side view) if the 2D slice proves too coarse.
- Calibrate clutter drag and mixing against a real cabinet measurement when one exists.

Open questions
- Which part properties carry the maker's temperature limits.
