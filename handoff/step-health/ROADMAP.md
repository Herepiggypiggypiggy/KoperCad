# STEP model health: roadmap

Scope: say plainly how well KoperCAD reads a STEP file, and read the colours and product names it carries. Pure Rust, no OCCT or pythonocc (see research/python-occ/SUMMARY.md steps 1-2).

Done
- `novo-interop::part3d::health`: `step_health`, `health_of_folder`, `markdown_report`, default folder `Documents\KoperCAD-tests\step-samples` (override `KOPERCAD_STEP_SAMPLES`).
- `novo-interop::part3d::style`: product names and colours (STYLED_ITEM chains, per face, mapped items).
- `kopercad-cli parts step-health [dir] [--report file.md]`.
- Parts window, Add CAD files panel: health line with colour swatch and name, details behind "?".
- Synthetic pack `crates/novo-interop/tests/data/step-pack/` (5 hand-made files).

Next
- Run Tadjiro's real Rittal files (wall box 1500.510, a mounting plate, a TS 8 rail) and fix the entities the report lists.
- Use the colour and name to colour the 3D mesh and pick door/plate sub-parts (needs the Rittal build thread's PR #1862 merged first).
- ZIP files are not opened by the report yet.
