# REL-POLISH handoff

- Both slices merged (#934, #959). Open follow-ups from review: `select` in `gpu.rs` ignores `WGPU_ADAPTER_NAME`/`WGPU_POWER_PREF` (document or honour); check dialogs and panels at 560x320 (only the top bar is tested); real-screen check of wire numbers on a dense page at fit and of a no-GPU PC on Windows (harness).
- Slice 1 (B9): `novo-canvas/src/declutter.rs` `SECONDARY_MAX_BOOST` 2 -> 3 and `candidate_size`; `widget.rs` `paint_labels` uses it as the pre-cull, so a 2 mm wire number (3.8 px at the fit zoom of a 1366x768 laptop) is lifted to the 9 px floor instead of vanishing. Canvas only: PDF export does not use the floor.
- Slice 2 (B12): merged as #959: `novo-app/src/gpu.rs` (adapter order real GPU, virtual, software), `MIN_WINDOW` 560x320 in `lib.rs` used by `fitted_size`, start-up error text in `main.rs`, width tests.
- B8: nothing to change; see ROADMAP.
- Resume: `cargo test -p novo-canvas -p novo-app`.

## Open items (2026-10-17)

- Needs a real Windows run (harness thread): no-GPU PC, 125/150/200% DPI, wire numbers on a dense page at fit, dialogs at 680x320 and 683x384 (only the top bar and "no overflow" are tested).
- Size-the-enclosure panel still covers half the Cabinet page at 683x384; Preferences now closes it (#995).
- Preferences clipping at 683x384 did not reproduce.
- The Cabinet bar scrolls horizontally in small windows, so right-hand buttons hide until scrolled (note for the small-window scenario).
- Other dialogs still default to 420 high; only Export PDF and the bench dialog have explicit heights.
- 3D "Mouse and keys" panel covers a quarter of the view by default (QA D-H-I): collapse by default, separate PR.
