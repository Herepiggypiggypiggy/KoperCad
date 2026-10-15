# Handoff: part geometry

Branch: claude/project-thread-wczi3g. Draft PR against main (see PROGRESS.md for the head SHA).
State: module `crates/novo-interop/src/part3d` complete with tests; clippy and tests green.
Resume: `cargo test -p novo-interop`; try a file with the `step_mesh` example.
Talk to: integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S, 3D architecture cse_01UMNPbP9STYPHPF8CDKoBbN, scene model cse_01B9cqnevwA8exsVRHF1ZFpw, Parts library cse_01Vba4yMwZ8vHadcR2jj9wFD.
Traps: maker files are user-imported only, never commit them (fixtures are synthetic). Orientation uses loop direction, not same_sense. Cache keys include the algorithm version; bump `ALGORITHM_VERSION` on any output change.
