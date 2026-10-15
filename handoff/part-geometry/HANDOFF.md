# Handoff: part geometry

Branch: claude/project-thread-wczi3g. Draft PR against main (see PROGRESS.md for the head SHA).
State: module `crates/novo-interop/src/part3d` complete with tests; clippy and tests green.
Resume: `cargo test -p novo-interop`; try a file with the `step_mesh` example.
Talk to: integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S, 3D architecture cse_01UMNPbP9STYPHPF8CDKoBbN, scene model cse_01B9cqnevwA8exsVRHF1ZFpw, Parts library cse_01Vba4yMwZ8vHadcR2jj9wFD.
Traps: maker files are user-imported only, never commit them (fixtures are synthetic). Orientation uses loop direction, not same_sense. Cache keys include the algorithm version; bump `ALGORITHM_VERSION` on any output change.
UI entry (not built): part sheet action "Import 3D model (STEP)" and mesh status line; see docs/design/part-geometry.md. UI threads: session_01MQYH2ZYReC8dTZsMHYf4pc (parts), session_01G52sVY5N34jPsrkqWukApR (cabinet/3D).
Renderer thread (session_01UMNPbP9STYPHPF8CDKoBbN) asked not to start the STEP importer before the owner decides (3d-view.md s12); Answered 2026-10-15: yes, wanted; #449 goes through normal review.
