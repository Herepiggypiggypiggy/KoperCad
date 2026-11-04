# STEP model health: handoff

Branch claude/project-thread-zf613j. New files only in `part3d/health.rs`, `part3d/style.rs`, `part3d/health/tests.rs`; one method `StepFile::names` in `parse.rs`; two `pub mod` lines in `part3d/mod.rs` (kept away from PR #1862's `body` line). App: `parts_cad.rs` (health cache filled in `read_identity` for STEP files up to 32 MB, line in the row). CLI: `parts.rs`.

Run on real files: put them in `Documents\KoperCAD-tests\step-samples`, then `kopercad-cli parts step-health --report health.md`. Files stay private; never commit them.

Regenerate the pack: `KOPERCAD_WRITE_STEP_PACK=<dir> cargo test -p novo-interop --lib write_pack_when_asked`.

"Unsupported" is a heuristic: geometry-looking kinds (`*_SURFACE`, `*_CURVE`, `*_BREP`, `*_SOLID`) that the reader does not list. Verdict Partial means gaps; colourless is a note only. Not yet seen with a real Rittal file.
