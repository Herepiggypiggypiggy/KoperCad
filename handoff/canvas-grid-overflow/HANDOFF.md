# Canvas grid overflow (PR #1957)
Fix: `GridIndex::cell_count` in `crates/novo-canvas/src/scene.rs` uses i128; `is_large` and `query` share it.
Tests: `grid_cell_count_does_not_overflow_on_full_i32_range`, `hostile_inputs_do_not_overflow_the_grid`.
Status: PR open, crash fix, no UI entry. #1904 and #1948 should drop their scene.rs changes and merge main.
