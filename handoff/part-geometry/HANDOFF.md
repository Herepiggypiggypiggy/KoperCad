# Handoff: part geometry

State: #449 (engine) and #506 (parser hardening, Fallback::message/can_retry) are merged. No open PR. Module: crates/novo-interop/src/part3d. Design: docs/design/part-geometry.md.
Resume: `cargo test -p novo-interop part3d`; try a file with `cargo run -p novo-interop --release --example step_mesh -- part.step`.
UI entry: Parts window "3D model" block (UI integration thread, session_01MQYH2ZYReC8dTZsMHYf4pc, PR #508). Entry points: GeometryService::new(MeshCache::open_default(), Quality::Normal, TIME_LIMIT, wake); request(GeometryJob), geometry(key), forget(key); Fallback::message()/can_retry().
Next, in order: (1) 3D view adapter: novo-view3d `import` feature implements GeometryProvider from PartGeometry (fields already match MeshData; renderer thread wants STEP parsing to stay in novo-interop). (2) Command parts.import_step for Ctrl+K. (3) "Building 3D models: 12 of 100" hint. (4) B-spline surfaces, spindle tori, per-part rotation override, generated shapes.
Talk to: lead cse_01FiLvRBKEcNNHbQEGhYyL1S, renderer session_01UMNPbP9STYPHPF8CDKoBbN, main reviewer C session_012GWMePFNbeNns8NbmSMaXJ.
Traps: maker files are user-imported only, never commit them (fixtures are synthetic). Orientation uses loop direction, not same_sense. Bump ALGORITHM_VERSION on any output change. earcutr is declared in novo-interop; move to [workspace.dependencies] when a second crate needs it.
