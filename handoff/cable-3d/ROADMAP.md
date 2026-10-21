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
2. Bend radius and clearance checks: needs a minimum bend radius on the cable part (`novo-parts`) and a hot-spot distance from the heat result. Until then the card says "Not checked".
3. Pick a duct in 3D and list the runs in it (reverse of the card).
4. Cable runs between two cabinets (two plates) once the model routes across enclosures.

## Open questions
- Duct names: the card says "Duct at y 415 mm" because rails have no tag. A duct tag in the model would read better.
