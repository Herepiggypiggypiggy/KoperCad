# connection-screens: notes for the reviewer

Teaches: the generated "Terminal diagram" page of a strip (what each column and the jumper bar show), that it follows the
drawing live (cables accepted in the Cables drawer, a terminal renamed). 11 steps, about 8 minutes, `draft`.
Project: terminals.rs `connection_screens_start`: strip +CAB-X1 with 8 terminals (1 2 3 PE 4 5 6 7), wire numbers 101.. / 201..,
terminals 5 and 7 bridged, pump cable -W1 (4 wires), two valve cables still proposed, generated pages 0 10 20 30 40 50.

Verified path per step:
1. goal: reading.
2. open-diagram: outline "Pages" toggle (app.rs:1535) or pinned panel (app.rs:999); row label "40  Terminal diagram · +CAB-X1" from backend.rs:700 `page_rows`.
3. read-diagram: sheet = novo-reports strip_diagram.rs `strip_sheet` (heads at :164, jumper bars :248), shown by app/reports.rs:63.
4. open-cables: Ctrl+4 (keymap.rs:392) -> workbench.rs:513; works over a generated page (drawers.rs:36).
5. accept-cables: button "Accept 2 proposed" drawers.rs:76 -> :153 `accept_cable_proposals`.
6. see-follow: LiveReports regenerates by dependency (novo-reports report.rs:247; diagrams.rs:15 lists Cable).
7. open-terminals: Ctrl+3 switches the tab (terminals.rs:149).
8. show-terminal: click the row name (terminals.rs:226) -> drawers.rs:117 `jump` (records history).
9. rename: DOUBLE-CLICK the terminal name (novo-editor view.rs:966) -> edit.rs:310 `rename_label`. F2 does not work on terminals.
10. back-to-diagram: Alt+Left = nav.back (keymap.rs:382) -> xref.rs:116.
11. done: reading.
Tests: `connection_screens_steps_hold_on_the_finished_project` (cables -W2/-W3, row 11, Cable column).

GAPs
- No check kind for a terminal (strip + name) and no solve operation `rename` or `cables.accept_proposed` (the button has no command id).
- Anchors missing (drawer.cables.*, drawer.terminals.row.*, outline.pages).
- Layout coordinates in the notes come from the printed layout of an A3 sheet; if the frame changes they move.
- The drawer's own text for an empty strip says "Place a terminal (I, terminal)"; fine, not used.
