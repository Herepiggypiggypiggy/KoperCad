# Crane model picker: handoff

Branch `claude/project-thread-4xl2v4`. Thread: "Build crane model picker and motion". PRs go by parity (odd to lead B, even to lead A).

Where things are:
- `crates/novo-view3d/src/crane_model.rs`: pure engine. `CraneModel` (solids as meshes in the file's axes), `Orientation` (up axis + quarter turns), `PartRole`, `plan()` (stand the model on the floor, one mesh per solid in the renderer's local frame), `scene()`, `hook_bounds()`. `crane::push_floor` is the floor and yellow line shared with the generic crane.
- `crates/novo-app/src/app/crane_model_load.rs`: reads the STEP file (novo-interop `placed_parts`, chord tolerance 1 mm, coarser when too complex), simplifies each solid to a share of 180,000 triangles, guesses roles from names. Runs on a thread (`Loading`).
- `crates/novo-app/src/app/crane_model_state.rs`: `ModelSlot`, what the window holds: `want` (what the project asks), `loaded`, `loading`, `failed`. `update()` is called every frame of the window.
- `crates/novo-app/src/app/crane_model_ui.rs`: the buttons in the window's top row and the "Crane model parts" window (floating on purpose: docked it folds away).
- `crates/novo-app/src/app/crane_setup.rs`: `ModelRef` in `CraneSetup`; keys `crane3d.model`, `model_up`, `model_turns`, `model_roles` (written only when roles were edited).
- Fixture: `qa/fixtures/crane-model/workshop-crane.stp` (synthetic, made by `tools/make_crane_step.py`), scenario `qa/scenarios/9610-crane-model-picker.qa`.

Traps: the project keeps only the path, never the file; a missing file shows "not read" and the generic crane. A renderer mesh key must differ per plan (key holds a plan counter) or the GPU cache serves a stale mesh. Real customer crane files are private: never commit them. Opt-in check: `KOPERCAD_CRANE_STEP=<file> cargo test -p novo-app --lib real_crane_file -- --ignored --nocapture`.
