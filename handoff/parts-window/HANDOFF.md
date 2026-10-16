# Parts window fix: handoff

Branch `claude/project-thread-yak59m`; code in `crates/novo-app/src/app/parts.rs` (`parts_ui`, `list_ui`, `detail_ui`) and `parts_quality.rs` (`tab`).

Cause of the old garble: each row was a `right_to_left` layout inside `show_rows`, which takes all remaining height, and the window wrapped its content in a scroll area so the panels had no height to fill. Now the window has no outer scroll (`.scroll([false, false])`), rows use `novo_theme::table`, colours come from `Palette`.

Tests: `parts::tests::clicking_down_the_list_selects_each_row_in_order_without_gaps`, `a_selected_part_shows_its_data_inside_the_window`.
Screenshots: scripts/screenshot.sh needs xdotool, openbox, Xvfb; Light 1920 was not captured.
