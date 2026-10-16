# Handoff: trunk duct and rail/duct UI

Branch `claude/trunk-duct-and-rail-duct-ui-wyjgch`, PR from main (see PROGRESS.md for the head SHA).

Decision: the planner adds the trunk (`plan_with_trunk` in `crates/novo-layout/src/rules.rs`), the router is unchanged. A router that bridged parallel runs would route wires across gaps no duct covers, which the cabinet cannot build and the cut list would not list. The trunk is a vertical class `mixed` duct on a strip kept free at the left edge of the usable plate; rows are packed again narrower; the row ducts start on its centre line so the router sees a joint. With fewer than two rows no trunk; if the narrower plate would lose a device the plan without trunk wins (`Explanation::trunk_dropped`).

UI: `crates/novo-app/src/app/cabinet_rails.rs`. Cabinet header "Rails and ducts", command `cabinet.rails` (View > Workspace, Ctrl+Shift+R). Click picks a rail or duct away from devices (ducts over rails), drag moves it (snapped), Delete removes it, the list edits position and size. Each edit is one undo step.

Tests: `novo-layout/tests/rules_v2.rs` (trunk), `novo-app` `route_all_tests.rs` (motor starter: all 11 cabinet wires ducted), `cabinet_rails_tests.rs`.

Round 2 (R3 review + Tadjiro could not move a rail): every rail and duct has a grip tab outside its start (`pick_grip`), grabbable even when devices cover the rail; hover outlines the rail and shows a grab cursor; arrow keys nudge the selected rail by the grid (Shift = 1 mm), the devices on it go along (`Tx::move_rail`). Up and Down arrive as the schematic's `terminal.strip_up/down` commands, so `app.rs` hands them to the Cabinet. `cabinet.rails` passes the Cabinet key filter.
