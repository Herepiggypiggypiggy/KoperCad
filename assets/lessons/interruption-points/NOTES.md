# interruption-points: notes for the reviewer

Teaches: an interruption point carries a signal from page 1 to page 2; the points find each other by name; G jumps; the
points are numbered; the coil shows the page.column of its contacts. 12 steps, about 9 minutes, `draft`.
Project: terminals.rs `interruption_points_start`: page 1 -F1/-S0/-S1 with -S1:14 open, page 2 coil -K1, page 3 contacts
13-14 (-H1) and 21-22 (-H2). Finished: points named 1/1, wires drawn, no checker findings, no cross-reference issue.

Verified path per step:
1. goal. 2. first-point: Q/I place field (keymap.rs:344), alias "ip" (catalog.rs:116); the wire to a facing pin is drawn on drop (tool.rs:645, default ConnectMode::Draw tool.rs:78).
3. next-page: PageDown (keymap.rs:385). 4. pick-second: workbench.rs:448. 5. turn: R rotates the piece in hand (tool.rs:1925).
6. second-point: IP2 is named by edit.rs:292. 7. same-name: double-click the name (view.rs:966) -> edit.rs:365; partners novo-core xref.rs:118.
8. jump: G (keymap.rs:380) -> app/xref.rs:74. 9. renumber: Ctrl+K "number interruption" -> workbench.rs:511 -> potentials.rs:34 -> novo-xref number.rs:45.
10. back: G again. 11. contact-image: label from novo-xref index.rs:429; Shift+G list xref.rs:101. 12. done.
Reference format: page.column, docs/DESIGN.md section 3 (ten columns on A3; x = 40 mm is column 1).
Tests: `interruption_points_steps_hold_on_the_finished_project` (names, labels "2.1" / "1.1" / "13-14 3.1" / "21-22 3.2", no issues).

GAPs and oddities
- No check kind for an interruption point or "no lone interruption" (problem ids are not published); count of symbols and an invented `problem_free = "lone_interruption"` stand in.
- solve: `place` has no rotation (`turns` proposed), no `rename` operation.
- ODD: the editor names new points IP1, IP2 (edit.rs:292) but novo-xref names them 1, 2 (number.rs:15) and renumbering turns IPn into n. Placing a second point never pairs with the first; the person must rename it.
- ODD: F2 does nothing on an interruption point or terminal (view.rs:1292 looks for a device), yet the right-click row says "Rename   F2" (view.rs:1734).
- ODD: `xref.link` can join two interruption points, but a selection never spans pages, so it cannot pair points across pages.
- The receiving point must be turned (R twice); there is no "continue from" placement that does it.
