# REL-POLISH handoff

- Slice 1 (B9): `novo-canvas/src/declutter.rs` `SECONDARY_MAX_BOOST` 2 -> 3 and `candidate_size`; `widget.rs` `paint_labels` uses it as the pre-cull, so a 2 mm wire number (3.8 px at the fit zoom of a 1366x768 laptop) is lifted to the 9 px floor instead of vanishing. Canvas only: PDF export does not use the floor.
- Slice 2 (B12): branch `claude/project-thread-kgjrte-b12`: `novo-app/src/gpu.rs` (adapter order real GPU, virtual, software), `MIN_WINDOW` 560x320 in `lib.rs` used by `fitted_size`, start-up error text in `main.rs`, width tests.
- B8: nothing to change; see ROADMAP.
- Resume: `cargo test -p novo-canvas -p novo-app`.
