# Two wiring modes: engine plan and test list

The behaviour is specified in docs/DESIGN.md (the wiring modes paragraph,
PR #169); this file holds only the engine plan, build order and tests. If
they differ, DESIGN wins.

## Engine

- `WiringMode { Corners, Lines }` on `Editor` (not on `WireTool`, which is
  rebuilt after every wire), with `set_wire_mode` and `toggle_wire_mode`.
  The app keeps it in `Settings.wiring_mode` (per user, persisted).
- `geometry::overlaps(a, b)`: shared stretch of positive length between two
  routes, axis-aligned in v1. An end that touches another wire's interior
  collinearly counts as overlap, not as a T-junction. A per-page segment grid
  keeps drags fast; a benchmark on the 10k-object project goes with the PR.
- One function, `edit::settle_wire(tx, page, wire)`, called from `draw_wire`,
  `accept` (facing proposals), `insert_into_wires`, `transform` and
  `tidy_junctions`; the move ghosts run the same step so the preview equals
  the commit. Deterministic, so undo and redo do not drift.
  - Router-chosen legs: flip or shift one grid step; at a pin the stub leaves
    along `endpoint_direction` for one step, then shifts.
  - Corners nudges (up to 3 snap steps); Lines never nudges and refuses.
    The refusal is typed data (`Refusal { other: WireId, stretch }`) returned
    by the engine entry, so tests assert on it; the UI draws it live
    (`Clicked::Refused`).
  - No merging of wires (DESIGN #169): any overlapping wire is nudged in
    Corners or refused in Lines. Same pin and direction: derived junction.
  - Move with no room: `settle_wire` takes one flag. Interactive (drag or
    keyboard nudge): refuse live (design reviewer decision). Non-interactive (scripts, AI edits, undo/redo, file
    open): accept and let `wire.overlap` report.
- `edit::draw_wire_exact` takes explicit points and does not route (Lines).
- `novo-checks`: `wire.overlap`, warning (as in DESIGN), for projects that already
  contain overlaps (raw `Command::AddWire`/`SetRoute`, imports). Core never
  rejects.
- Headless hooks for the tester: readable `wiring_mode` on the app and editor,
  mode loaded from Settings (not the project), and a tool-event API on the
  editor to inject press/drag/release.
- Keyboard thread: Shift+W (key repeat ignored), `EditorKey::Backspace`,
  command `wire.mode` with aliases eplan, autocad, corners, draw. Enter
  finishes a Lines wire only when no proposal is highlighted.

## Build order

1. This PR: this file (DESIGN text is #169).
2. `geometry::overlaps`, `settle_wire`, wired into the existing (Corners)
   paths, with tests.
3. `WiringMode`, Lines behaviour, `draw_wire_exact`.
4. `wire.overlap` check.
5. UI: setting, dock switch, hint text, Shift+W and `wire.mode`.

## Tests (run in both modes unless marked)

1. Aligned pins: identical route in both modes.
2. EPLAN journey (Corners mode): click pin, two corners, click pin; legs routed,
   all orthogonal. Backspace removes the last corner.
3. AutoCAD journey (Lines mode): click pin, three clicks, click pin; exactly the
   axis-locked segments, no router; Backspace removes the last segment.
4. A pin-to-pin drag gives the same wire in both modes.
5. Corners: an overlapping leg is nudged and the preview shows the nudge
   before the click. Lines: the overlapping segment is refused with a typed
   reason, nothing is placed, undo depth unchanged.
6. A route through another conductor's pin or junction is refused.
7. Two wires leaving one pin: derived junction, potentials and cross
   references unchanged, one undo step.
8. Two wires of one conductor that would overlap are handled like any other
   overlap (nudge in Corners, refuse in Lines); no merge.
9. T-junction onto a different declared potential: refused with the reason.
10. Crossing: no dot, no overlap report.
11. Move a symbol, preview equals commit, one undo restores. Interactive
    move that no nudge can clear: refused live, symbol stays, undo depth
    unchanged. Non-interactive (script, AI edit, undo/redo, file open): the
    result is accepted and `wire.overlap` reports it.
12. Shift+W mid-wire keeps the points; OS key repeat does not flip it back;
    the setting survives a restart.
13. A click under 4 px of movement is never a drag. The refusal tint and hint
    show live, before the click. Esc mid-wire cancels the whole wire.
14. A project with an overlap loads unchanged and shows one `wire.overlap`.
15. Property test: after any sequence of gestures in either mode, no two
    wires on a page overlap (except as flagged by test 11 and 14).
