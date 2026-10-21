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
1. Outside part of a run: a drawn stub from the entry along the gland's wall once the model knows which wall the entry is on (today the entry is a plate point; the outside part stays a stated length, never drawn).
2. ~~Bend radius and clearance checks~~ done in PR 3 (see below).
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

## Done in PR 3 (branch claude/cable-bend-clearance)
- `novo_routing::checks`: `tight_bends` (corner of two ducts against the arc the wire needs, bisection on a sampled arc tangent to both middle lines) and `near_hot` (distance from the routed centre line to a device outline). Pure geometry, unit tested.
- Rules `wire.bend_factor` (6 x diameter), `wire.hot_gap_mm` (20), `wire.hot_loss_w` (5), planning values, in the Cabinet rules list.
- Run card: bend line (cable outer diameter from the cable part's `outer_diameter_mm`, else the wire's conductor with insulation), tight corners in red with both radii, close hot devices with their loss and the rule, and a last line saying what is still not checked. Tight corners get a red cross on the lit path.
- Honest limits (also on the card): no cable stiffness, no temperature rating, no real surface temperature, corners at terminals and in free air are not checked; the loss is the Heat view's figure (maker or estimated).
