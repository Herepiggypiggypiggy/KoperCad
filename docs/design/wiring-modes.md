# Two wiring modes (WS-wiring-modes)

Status: proposal, open for the Workflow, Layout, editor and keyboard threads.
Requested by Tadjiro on 2026-10-15.

## What the owner asked for

- **Corner mode** (EPLAN style): the user places corners and the lines between
  them connect by themselves.
- **Line mode** (AutoCAD style): the user draws the lines.
- Both are available; the user switches with a toggle in a readily available
  place and an easy, meaningful shortcut.
- Lines never overlap, in either mode.
- Where the toggle sits and which key it gets is decided by the Workflow and
  Layout threads.

## Terms

A wire is still one `Wire` in novo-core: two endpoints plus `route`, the bend
points between them. The modes differ only in the gesture that produces the
route. The stored model, undo (one gesture is one step), potentials and
cross references are identical, so a project drawn in one mode edits the same
in the other.

This does not reopen "no manual corners or T-pieces" (WS-08 decision). Corners
here are bend points of a wire, as `WireTool.corners` already is. Corner and
T-junction *pieces* are still never placed; junctions stay derived.

## The two gestures

Both start from the wire tool (W, or dragging from a pin) and end on a pin,
junction or wire.

| | Corner mode (default) | Line mode |
| --- | --- | --- |
| Click in empty space | Adds a corner; the router joins the previous point to it with at most two bends (`geometry::route_through`) | Commits one straight segment from the previous point, locked to the axis the pointer is nearer to |
| Preview | The routed legs to the pointer | The single locked segment to the pointer |
| Finish | Click a pin, junction or wire | Click a pin, junction or wire; Enter finishes on the last point when it is on a pin/wire; double click likewise |
| Backspace | Removes the last corner | Removes the last segment |
| Esc | The existing ladder: cancels the wire in progress | Same |
| Drag from pin to pin | Auto-routed (unchanged) | Auto-routed (unchanged): a drag has no intermediate points to draw |
| Hold Shift | Not used | Releases the axis lock for a 45 degree diagonal (sheets are mostly orthogonal; diagonals are rare and drawn on purpose) |

Corner mode is today's behaviour, so nothing existing changes for a user who
never switches. Line mode needs an engine entry that takes explicit bend
points and does not route (`edit::draw_wire_exact`).

## No overlapping wires

Rule: on one page, no two wires share a stretch of line of positive length.
Wires may cross (a crossing is not a connection and gets no dot) and may meet
at a junction or a pin.

The rule is enforced in the editor engine, not in the UI, so it holds in both
modes, for drags, for moves, and for edited wires.

- **Detecting:** `geometry::overlaps(a_route, b_route)` returns the shared
  stretches (collinear, parallel, overlap longer than EPS). Pure, tested.
- **A new or edited wire that would overlap** is nudged: the overlapping run
  moves one snap step (2.5 mm) sideways, away from the existing wire, keeping
  the ends on their pins. If no nudge clears it within three steps the gesture
  is refused with a notice naming the other wire, and nothing is changed. The
  engine never joins the two wires electrically on its own; that would be a
  silent change to the connections.
- **A wire that runs along another from a shared pin** (both leave the same pin
  in the same direction) is the one case where overlap is a fact of the
  topology. It is resolved by deriving a junction at the point they diverge:
  the stretch is shown once. This is the existing T-junction rule, not a
  second wire on top of the first.
- **Moving a symbol** re-runs the check on its wires; the result is one undo
  step with the move.
- **Imported or older projects** may contain overlaps. novo-checks gets a
  `wire.overlap` problem (severity warning, location at the stretch) with the
  nudge as its fix. The editor does not rewrite the file on load.

## The toggle

State: `WiringMode { Corners, Lines }` lives on the editor (not in the model)
and is a per-user preference stored in `novo-app` `Settings.wiring_mode`
(persisted like `outline_pinned`). It is not a project property: two people on
one project may draw differently.

Proposal for Workflow and Layout to answer:

1. **Shortcut: Shift+W** toggles the mode, from anywhere on the drawing. W
   starts a wire, Shift+W switches how wires are drawn, so the pair is easy to
   remember. It works while a wire is in progress: the points drawn so far are
   kept and the next click follows the new mode. It does not clash with the
   keys in DESIGN section 4 (Shift+G, Shift+P, Shift+O, Shift+F8 are taken;
   Shift+W is free).
2. **Where it is visible:** a two-state segmented control "Corners | Lines" in
   the tool dock directly under the wire tool, shown only while the wire tool
   is active or the drawing has no tool, plus the same word in the status
   strip while a wire is in progress. The control shows the key in its
   tooltip.
3. **Command field:** `wire.mode` ("Wiring mode: corners / lines") so it is
   discoverable by typing "mode" or "lines".
4. Switching never changes existing wires.

If the Layout thread prefers a different placement, only item 2 moves.

## Build order

1. This design PR (docs only).
2. Engine PR 1: `geometry::overlaps` plus tests, and the nudge, wired into
   `edit::draw_wire`, `edit::transform` and route edits. Holds in the current
   (corner) behaviour first.
3. Engine PR 2: `WiringMode`, `WireTool` line behaviour, `draw_wire_exact`,
   Backspace, Shift for diagonals, tests.
4. novo-checks PR: `wire.overlap`.
5. UI PR (after Workflow and Layout answer): `Settings.wiring_mode`, the
   segmented control, status text, Shift+W in novo-workflow keymap and
   `wire.mode` command, DESIGN.md section 4 row.

## Test list (for the tester and both reviewers)

Run each journey in both modes unless marked.

1. Straight wire between two aligned pins: identical route in both modes.
2. Pin to pin with an offset: corner mode routes itself; line mode with two
   clicks gives exactly the clicked axis-locked segments.
3. Three corners / three segments, then Backspace removes the last one.
4. Draw a wire along an existing wire: result is nudged, never overlapping;
   with no room, refused and unchanged (undo stack length unchanged).
5. Two wires leaving the same pin together: one shown stretch, a derived
   junction where they split.
6. Cross two wires: no dot, no overlap report.
7. Move a symbol so its wire would run over another: overlap-free after the
   move, one undo restores both.
8. Toggle with Shift+W mid-wire: points kept, next click follows the new mode;
   the setting survives an app restart.
9. Load a project with an overlap: unchanged on load, one `wire.overlap` issue,
   its fix removes it.
10. Line mode with Shift: a 45 degree segment; every other path stays
    orthogonal.
