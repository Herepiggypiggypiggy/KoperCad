# Layout presets: handoff

Code: `crates/novo-app/src/app/layout_presets.rs` (+ `_tests.rs`), hooks in `workspace.rs` (switch_workspace), `app.rs` (follow_window_size each frame, store_layout in `save`), commands in `novo-workflow` (commands.rs, menus.rs, language.rs).

Model: Slot (Draw, Review, Cabinet, ThreeD, Reports, Commissioning) belongs to a workspace; Draw and Review both are the schematic. SizeClass comes from window width in points (so display scaling is included), with a 24 pt hysteresis. Layout = outline pin and mode, drawer tab, inspector pin. Stored in `Settings.workspaces` under `<slot>/<class>`; a plain workspace id is the pre-size fallback.

Traps: tests that call `follow_window_size` then `d.frame()` get the harness width back; set `app.size_class = None` to force a re-apply. Cloud disk fills during workspace builds: delete target/debug/incremental.

Height class (follow-up PR): `SHORT_BELOW` = 800 pt, `next_short`, `follow_window(width, height)` (`follow_window_size` is the width-only wrapper and uses `window_height`), key `<slot>/<class>/short`, `layout_of` falls back short -> same width -> legacy key -> default. `window_short` and `window_height` live on `NovoApp`. Tests that call `follow_window_size` after `d.frame()` get the harness height, which is tall.

Floating panel positions (follow-up PR): `WorkspaceLayout.panels` (id -> [x, y] points, moved panels only). `panel.rs` records the panels it shows (`note_seen`), `panel::places(ctx)` reads the moved ones from egui's area memory, `panel::place_all` queues a placement per panel that `Panel::show` takes on its next frame. `NovoApp::sync_panels` runs once a frame before `follow_window_size`; `panel_places` mirrors the panels for `store_layout`, `panel_pending` carries a layout's positions in `apply_layout`. Trap: egui 0.36 ignores `current_pos` for a title-bar-dragged window (it restores the pre-begin position), so the placing frame uses `WindowDrag::Anywhere`. Sizes are not kept per layout.
