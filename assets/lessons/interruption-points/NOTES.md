# interruption-points: notes for the reviewer

Teaches: an interruption point carries a signal from page 1 to page 2; the points find each other by name; G jumps; the
points are numbered; the coil shows the page.column of its contacts. 10 steps (the separate "turn" step was merged into
second-point), about 9 minutes, `status` unchanged (`draft`), `version` 2.
Project: terminals.rs `interruption_points_start`: page 1 -F1/-S0/-S1 with -S1:14 open, page 2 coil -K1, page 3 contacts
13-14 (-H1) and 21-22 (-H2). Finished: points named 1/1, wires drawn, no checker findings.

## Replay

crates/novo-app/src/app/lesson_verify_terminals.rs `interruption_points_steps_work_in_the_window`: real key events (Q tap with
release, typed text, Enter, Esc, PageDown, R, G, Shift+G), real clicks at world positions, a real double click on the
name label (found with the canvas's own pick, so the position is where the canvas picks it), Ctrl+A, the command field.
Asserted per step: IP1 at (40,120) and +1 wire; PageDown shows "Contactor coil"; Q alone does not arm the tool; R, R gives
rotation R180; the click adds IP2 and the wire to -K1:A1; after the rename both points are IP1; G lands on "Start signal" and
back; "number interruption" renames both to 1; the coil's contact image reads "13-14 3.1" and "21-22 3.2" and Shift+G lists >= 3 places.

## Verified path per step (paths under crates/)

1. goal: reading.
2. first-point: Q/I tap opens the place field (keymap.rs, novo-editor view.rs); alias "ip" (catalog.rs); wire to the facing pin on drop; the place tool stays armed, Esc (edit.escape) ends it; check `count symbol >= 5`.
3. next-page: PageDown = nav.next_page; check `page`; no `point` (see below).
4. pick-second: Q, "ip", Enter; check `tool = place` (false after PageDown, true after Enter; Q alone does not tick it).
5. second-point: R twice on the piece in hand, click at (40,40), Esc; check `count symbol page 2 >= 3`.
6. same-name: double click the name label, Ctrl+A, IP1, Enter -> edit.rs `rename_label`; check `wire -S1:14 .. -K1:A1` (one net only when the two points share a name).
7. jump: G = nav.partner -> app/xref.rs `go_partner`; check `page`.
8. renumber: Ctrl+K "number interruption" -> app/potentials.rs -> novo-xref number.rs; check `command interruption.renumber`.
9. back: G again; check `page`.
10. contact-image: click the coil, Shift+G = nav.all_references -> app/xref.rs `list_occurrences`; check `command`.
11. done: reading.

## Instructions the program REFUSED or got wrong, and the fix
- next-page ticked the moment it opened: its `point` was on page 2 and the runner switches the view to a target on another page when the step opens. The `point` is removed from every step that changes page (next-page, jump, back).
- same-name ticked the moment it opened: `problem_free = lone_interruption` is always true (the engine never fills the problem set, and the real id is `check.lone_interruption`). It is now `wire = { from = "-S1:14", to = "-K1:A1" }`, which is false until the two points share a name.
- pick-second ticked on the first press of Q (`command tool.place`); it is now `tool = place`, which is true only after the words and Enter.
- turn ticked on the first R of two (`command edit.rotate`); the step is merged into second-point, whose check is the placement.
- Double-clicking the name while the place tool is still armed does not rename (it places again); the text now ends first-point and second-point with Esc.
- The "tap, do not hold" caveat for Q (a hold opens the wheel) is in the text.

## GAPs closed since the first version
- solve `place` has `turns` (used for the half turn).
- `interruption.renumber` works as described (two points become 1, 1).

## GAPs still open
- No check kind for "an interruption point exists" or "turned half a turn": symbol counts stand in.
- `Facts.problems` is never filled, so `problem_free` is true from the start; the wire check replaces it here.
- No solve op `rename`; same-name's `solve` is `xref.link`, which cannot pair points on two pages (a selection never spans pages; checked again on current main), so Show-me/replay cannot complete that step (`# GAP solve`).
- ODD: the editor names new points IP1, IP2 while renumbering uses plain numbers; a second point never pairs with the first by itself.
- ODD: F2 does nothing on a point or terminal although the right-click row says "Rename   F2".
- The rename box does not preselect the name (Ctrl+A needed).
- No anchors for the place field or the palette field (palette.button is used).

## How lists

The `how`, `how_nl` and `demo` lists are in lesson.toml (reading steps have none).

## Where the person ends (jump, renumber, back)
Checked by lesson_verify_terminals.rs `interruption_points_steps_work_in_the_window` (keys) and
`interruption_points_solves_end_on_the_pages_the_text_names` (the lesson's own solve commands): jump ends on page 1
"Start signal", renumber leaves the view on page 1 with the point selected, back ends on page 2 "Contactor coil".
A solve-only replay cannot reach this because same-name has no rename solve op (`xref.link` does not pair
interruption points): GAP solve rename.
