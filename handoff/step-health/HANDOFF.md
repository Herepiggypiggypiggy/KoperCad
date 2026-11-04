# STEP health report: handoff
- Engine: `crates/novo-interop/src/part3d/health.rs` (`check_step`, `StepHealth`, `decode_text`). CLI check: `cargo run --release -p novo-interop --example step_health -- file.stp`.
- UI: `crates/novo-app/src/app/parts_health.rs`, wired in `parts.rs` under 3D model.
- Real-file results (Tadjiro's three Rittal downloads, kept private in /mnt/project-files/private/rittal-step/, never in the repo): all three read. Plate 175x285x2 clean. TS 8 rail 744.8x84x23, 3 solids, clean. Enclosure assembly 21 solids, 1038 faces, 12 approximated (2 are DEGENERATE_TOROIDAL_SURFACE, not supported), 1 skipped. Size 1150x1860x883 is the whole assembly, not the cabinet.
- Colours: the files carry only grey styles per solid (#b0b0b0, #808080, #585858, #bfbfbf), no face colours. Product names on the enclosure are internal codes ("88X5_00_01_ES (2)"), so door/plate cannot be picked by name; use size/geometry (next step).
- Synthetic tests: `crates/novo-interop/tests/part3d_health.rs`.
