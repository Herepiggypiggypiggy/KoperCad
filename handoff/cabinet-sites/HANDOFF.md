# Handoff: cabinet sites

Branch `claude/project-thread-h0jsv8` (PR 1). Engine in `crates/novo-layout/src/site.rs`, `model.rs` (`belonging`, `in_the_field`, `off_plate_devices`, `power_class_size`), `surface_rules.rs` (standard door rules), `sizing_model.rs` (`sizing_devices`, `left_out`). Check in `crates/novo-checks/src/physical.rs`. UI: `cabinet_surface_move.rs` (`show_site`, `set_site`), `sizing.rs`.

Decisions:
- Letter code defaults: M, Y are field. S, H, P are door via the surface rules (an enclosure with its own `surface_rules` text keeps it; an empty text opts out).
- No door or no inside size: door devices stay on the plate and Place all says so.
- The shipped example keeps its two push buttons on the plate (`mounting_site=plate`) so every wire still routes; the EMC lesson project was regenerated for that.
- Disk: build with `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`, one crate set at a time.

Traps: `Spec` has a new field `surface`; tests that build `Spec` literally need `surface: None`.
