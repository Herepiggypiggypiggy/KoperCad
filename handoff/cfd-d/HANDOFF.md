# CFD-D radiation: handoff

Branch `claude/cfd-d-radiation-nlu47i`, repo kopercad, PRs from main. Reviewer R1 `cse_01NCdhCktKEbKPHFBvi2Tjsp`, lead A `cse_01FiLvRBKEcNNHbQEGhYyL1S`, coordinator `session_012zgp6RwwaD938gARNwqHHY`.

Code: `crates/novo-cfd/src/rad/` (`patches.rs`, `trace.rs`, `viewfactor.rs`, `radiosity.rs`, `paths.rs`, `mod.rs` with `RadSettings` and `RadiationModel`). Tests: `crates/novo-cfd/tests/rad.rs`.

Model decisions
- Cell is solid when its dominant material is not air (same as `Mesh::material`); cut-cell faces are taken whole (first order). Put test bodies on grid planes.
- Patch key: kind, material, direction, plane index, block of cells; block grows until at most `max_patches` (default 1500, dense n x n matrix).
- Rays: cosine-weighted, Halton with per-patch shift, deterministic for a seed. Raw errors are kept in `ViewFactors::stats`.
- Emissivity values are planning values (plate 0.3, devices 0.9, walls 0.9); override per material id.
- Air is transparent.

Resume: `cargo test -p novo-cfd --test rad`, `cargo clippy --workspace --all-targets -- -D warnings`.
