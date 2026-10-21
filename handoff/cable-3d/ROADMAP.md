# Cable runs in 3D: roadmap

Scope: draw cable and wire runs in the 3D cabinet view along the routed paths to the cabinet entry, and read each run: path, length basis, duct fill, problems. Source: audit/creative/round7.md idea 35, the cabinet entry lane (#1160/#1163/#1164).

## Done (PR 1, branch claude/project-thread-168e6g)
- Run card (click a wire in 3D, or a row of the Wire runs list): length with its basis in the wire list's words, why that basis, inside-the-cabinet length, path (ducts, bends), fill per duct, entry, cable, problems, what is not checked.
- Lit path and duct outlines (fill colour) for the picked wires; cabinet entry marker (diamond, cross and post) whenever the plate has an entry point.
- "Select all cores of the cable".
- Wire runs list: collapsed "Not drawn (n)" section with each undrawn wire's length, basis and reason.
- Hover card carries the basis, the entry line and ducts that need attention.
- QA scenario `qa/scenarios/31-cable-runs-3d.qa`; app tests `view3d_runs_tests.rs`.

## Next, in order
1. ~~Outside part of a run~~ done in PR 5 (see below).
2. Bend radius and clearance checks: needs a minimum bend radius on the cable part (`novo-parts`) and a hot-spot distance from the heat result. Until then the card says "Not checked".
3. Pick a duct in 3D and list the runs in it (reverse of the card).
4. Cable runs between two cabinets (two plates) once the model routes across enclosures.

## Open questions
- Duct names: the card says "Duct at y 415 mm" because rails have no tag. A duct tag in the model would read better.

## Done in PR 2 (branch claude/cable-runs-panels)
- One Wire runs panel (list when open, Run card of the picked wire below) on the right under the view cube, via `panel::Panel`: follows the canvas, draggable, no overlap with the Inspector (left) at 1366x768, 1920x1080 and 1366x600; starts under the Heat panel when that is open.
- Review follow-ups of #1205: cable shorter than the inside route has its own sentence; stale `fill_basis` comment fixed; the basis is read only after the project has been unchanged 0.25 s (no re-route per drag frame); "cable length" is neutral, not green; the card says fill is a planning value.
- Ctrl+K: "cable run", "cable runs", "run card", "route", "cabinet entry" find View > 3D > Wire runs.

## Still open
- Heat panel and Wire runs panel both hang under the cube: with Heat open on a short window the Wire runs panel starts beside it (draggable).
- "option" in Ctrl+K returns scattered letter matches (round 11): fuzzy ranking in `novo-workflow`, the UX thread's.

## Done in PR 5 (branch claude/cable-entry-stub)
- Property `entry.wall` (top, bottom, left, right) on the enclosure, no `novo-core` change: `novo_routing::entry::{Wall, entry_wall, set_wall, nearest_wall, edge_point}`. Not stated reads as the nearest plate edge, flagged as a guess.
- Cabinet > Routes panel: "Gland in the" selector (Nearest edge, Top, Bottom, Left, Right), one undo step.
- 3D: the entry marker gets a line from the top of its post to the plate edge of the wall and an arrow beyond it, solid when the wall is stated and dashed when it is the nearest edge by default. The outside part is a stated length, not a path, so the arrow is not to scale (the Run card says so).
- QA scenarios 39 (1366x768) and 39-1920 (1920x1080).
- No length uses the wall; it only decides where the way out is drawn.
