# Chain: where each downstream feature gets its enclosure data

Goal (Tadjiro 2026-11-04): simulations and everything after run on the ACTUAL enclosure chosen at the start.
Today most features take typed numbers or defaults. Source = code read in main 210c4ce. "Change" is what the
`EnclosureSpec` (see DESIGN.md) must feed. Each row is its own PR/thread so no scope shrinks.

| Area | Where it takes enclosure data today | Change needed |
| --- | --- | --- |
| Cabinet canvas and plate | `Structure.plate` (W, H, margin), typed in the cabinet bar or CSV catalogue | Plate from the chosen plate article; usable area from real holes and cut-outs |
| Depth | property `depth_mm`, 250 mm default in 3D view | From spec outer/interior depth; remove the silent default for chosen enclosures |
| Door and swing | `novo_layout::Enclosure` set by hand in `cabinet_door.rs` (interior, setback, hinge, angle) | Fill interior, setback, hinge, angle, inner depth from spec; dialog edits only override and mark `typed` |
| 3D view | `novo-view3d` builds a box shell from plate + depth (`build/model.rs`) | Real body, door, roof, plinth from spec; STEP geometry when given (render only, never stored) |
| Heat / CFD | `novo-thermal` `bridge.rs`: outside size, wall, material, installation typed in heat settings; shell layers #1865 | Outside size, wall, material and layers from spec; openings and fans at real positions |
| Vents, fans, filters | `heat_openings.rs`, `view3d_vents.rs`: wall + centre typed or companion defaults | Real cut-outs from the STEP body; accessory articles (fan, filter) as openings |
| Cooling selection | `sizing.rs` + `EnclosureCatalog` CSV (effective surface from typed outside size) | Effective surface from spec outer size and installation; Rittal cooling article suggestions |
| Routing and duct fill | `novo-routing` network from rails/ducts on the plate; duct sizes typed | Factory ducts and rails from spec; real duct cross-sections; entries from gland-plate openings |
| Wiring lengths | routing result + plate coordinates | Add door-side and wall-entry segments from the spec; setback for depth moves |
| Drill and cut output | `workpack.rs`, #1862 drilling plan for a maker plate | One output from plate holes, enclosure cut-outs and device footprints; marks which holes are factory |
| EMC | `novo-emc` reads rails, ducts, devices; geometry from plate | Shell material, seams, gland plates and cable entries from spec (shielding indices) |
| Mechanical checks | `novo-layout` fit checks (`check`), door report | Usable depth, plate size and weight limits from spec; weight vs load rating from catalogue |
| BOM and part list | `novo-parts` list; enclosure appears only if the user adds it | Enclosure, plate, rails, roof, plinth, door lock as parts with article numbers on create |
| Reports and handover | Reports read structures and devices | Cabinet sheet: article, size, wall, plate, provenance per field (catalogue / step / measured / typed / default) |
| Commissioning and photos | `commissioning_cabinet.rs` uses plate layout | Reference the enclosure article and door/front view |
| Simulation (`novo-sim`, machine) | no enclosure input | None; enclosure matters only through heat |
| AI and CLI | `novo-ai` engineer cabinet, `kopercad-cli` take plate sizes | Offer the catalogue and the spec fields; report provenance |
| AML / STEP export | `step::export_enclosure` writes a box from plate + `depth_mm` | Export outer size and wall from spec; mention the maker article |

## Principles

- One record, many readers: no feature keeps its own copy of the enclosure.
- Read from the spec through one query function in `novo-layout` (pure), not through app code.
- A chosen enclosure with a missing field fails loudly in the checks panel (one line), never falls back silently
  to a default size or 250 mm depth. Old projects without a spec behave as now.
- Provenance (`catalogue`, `step`, `measured`, `typed`, `default`) travels with each number into heat and reports.

## Proposed per-area threads (to be started by the coordinator after E1 merges)

Heat and CFD (spec to thermal, with the heat researcher and #1865); Door and 3D shell; Routing and duct fill
with wiring lengths; Drill/cut/BOM/reports; EMC and mechanical checks; Cooling selection. Each reads CHAIN.md and
adds a test that changes the spec and sees the result change.

## Record requirements from the heat shell lane (#1856, heat-shell-ui.md section 7)

1. Provenance per field: `catalogue`, `step`, `measured` (only when the user marks it), `typed` or `default`, saved with the project. Heat results quote the weakest source of the fields they used.
2. Envelope for the 3D heat view: body mesh or parametric ribs, roof, plinth and door swing.
3. Installation data read by the wiring lane: installation, plinth, covered faces, finish, absorptance; `default` when not set (no silent default).

## QA pictures (1366x768 and 1920x1080, from the exact head)

Every PR names the scenario it adds. Pictures use synthetic enclosures, never a Rittal file.

| Step | PR | Pictures |
| --- | --- | --- |
| Panel opens from Start screen, Cabinet empty state, File menu | E3 | each entry; panel with the estimate line |
| Create makes the cabinet; undo removes it | E3 | before, after, after undo |
| STEP drop; mismatch shows both numbers | E4 | drop target, result line, mismatch |
| Heat, door, routing, drill, EMC, BOM follow a changed spec | E5.. (one per area) | spec A against spec B, side by side |
