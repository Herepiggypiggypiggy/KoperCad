# Panel building: what EPLAN Pro Panel does and what KoperCAD should take

## Re-evaluation 2026-10-22 (read this first)

Status: second pass for the owner, research only, no code. Sections 1 to 10 below are the 2026-10-15 original; its
ranks R1 to R9 have mostly landed, so this section replaces section 4 (matrix) and section 6 (ranking). Where a row
below disagrees with the original, this section wins.

**How sure each statement is.**
- *Source*: stated on an EPLAN public page (listed in "Sources 2026-10-22" at the end of this section).
- *Inferred*: my reading of what the pages imply; EPLAN does not spell it out.
- *Not confirmed*: pages are silent. Not hands-on: nobody here has run Pro Panel.
- KoperCAD "Done" means code and tests on main (checked against `crates/` and `handoff/*/HANDOFF.md`), not a hands-on pass on
  Windows. "Model" means a planning estimate, not validated against a tool or standard.

Owner context: wires are cut and holes drilled by hand. That makes assembly guidance, printed output and part data worth more
than machine formats. No EPLAN macros, libraries, manual text or motor data were used; the customer-derived notes in
`interop/` were read for context only and nothing from them is quoted here.

### What Pro Panel does, now (2026 state)

Pro Panel builds a 3D digital twin from the schematic: enclosure and plate, rails, ducts, devices with the maker's real
dimensions (Data Portal), drill holes and cut-outs, routing rules and wire lengths with duct fill, clearance and thermal
checks, parts, cutting and wiring lists, and NC data for machining (Rittal Automation Systems) [source]. Wire production data
goes to wire machines (Rittal WT, Steinhauer, CadCabel, Metzner, Schleuniger, Komax), one licensed format each; the generic
export lists sequence, colour, cross-section, both ends, length, end processing and direction per end, bundle number,
connection number and the ducts passed [source]. Smart Production adds step-by-step browser work instructions with 3D, drill
patterns, wire routes, traffic-light status, questions back to design, change support between versions, order management
and a dashboard [source]. Pro Panel 2026 added up to ten saved 3D views per project and easier routing configuration
[source]. Smart Mounting and Smart Wiring pages did not load in detail; their content here is *inferred* from Smart Production.

### Capability table

| # | Capability | Pro Panel | KoperCAD | Notes |
| --- | --- | --- | --- | --- |
| 1 | 3D layout space from schematic devices | Source | **Done** | Cabinet workspace plus 3D view (`novo-view3d`), unplaced list, place all with rules and trunk duct |
| 2 | Maker enclosures and accessories from a configurator (Rittal RiPanel) | Source | **Partial** | Enclosure sizing, door, plate, surfaces exist; no catalog of enclosures, plinths, partitions, accessories with real 3D |
| 3 | Real part dimensions and 3D models at scale (Data Portal) | Source | **Partial** | Bulk import, BMEcat, maker packs, STEP boxes and meshes, pins from STEP exist; shipped catalog is still tiny; import must run on the owner's PC (PARTS-REAL) |
| 4 | Rails and ducts, cut to length | Source | **Done** | Rail/duct UI, stock optimiser with kerf and offcuts (`stock.rs`), cut list |
| 5 | Mounting on plate, door, wall, profiles | Source | **Partial** | Surfaces engine and picker (PANEL-B1/B3); slices of the surface view still landing |
| 6 | Clearance and collision checks | Source | **Done** (2D, boxes) | Per-part rules, depth, door swing, 3D drag gaps. No mesh-level collision |
| 7 | Assembly accessibility (tool access, wire entry) | Not confirmed | **Missing** | Clearance zones exist; no screwdriver or wire-bend access check |
| 8 | Holes and cut-outs from placement | Source | **Done** | Drill table, cut-out cards, plate drawing |
| 9 | Associative drilling templates and mounting drawings | Source | **Done, printed 1:1** | Workshop pack: tiled 1:1 templates with scale check. Accuracy on paper is an estimate (about 0.5 mm), not measured |
| 10 | Machine NC data (Perforex, Steinhauer) | Source, licensed | **Missing, by choice** | Only worth it once a machine exists; hand drilling today |
| 11 | 3D wire routing, lengths, wiring holes | Source | **Done** | `novo-routing`, route all, entry legs, openings, lengths with slack and basis flag |
| 12 | Duct fill | Source | **Done** | Engine, check, 3D view |
| 13 | Wire production list (sequence, bundle, end processing, routing track) | Source | **Partial** | Wire cut list, job CSV, ferrule table, strip lengths from standard. Bundle number, per-end routing direction and machine sequence are not there (check the CSV columns before building) |
| 14 | Wire machine exports (Komax, Schleuniger, Rittal WT, ...) | Source, licensed per format | **Missing, by choice** | No machine named; ask first |
| 15 | Thermal: loss total, loss map, cooling selection (RiTherm) | Source | **Done beyond, one gap** | IEC 60890 estimate plus own 2D/3D CFD and heat views (model, validation in `docs/thermal-validation.md`). Missing: pick a fan or cooling unit from a catalog |
| 16 | Busbars in loss and layout | Source (loss only) | **Not checked** | Copper handling exists in sim; no busbar layout confirmed on main |
| 17 | 2D documents from the 3D model | Source | **Done** | Plate drawing, PDF, DXF |
| 18 | Reports: BOM, cut list, wire list, labels, ferrules | Source | **Done** | Bench packet bundles them; label sheet stays out by owner choice |
| 19 | Step-by-step work instructions with 3D, status, change support | Source (Smart Production) | **Partial** | Assembly checklist (PDF) and Bench packet exist; no interactive, tablet-friendly build mode with status |
| 20 | Wiring guidance per wire (route shown, sequence) | Inferred (Smart Wiring) | **Partial** | Cable runs lit in 3D with card and list; no wire-by-wire work order or bundles |
| 21 | Questions from the shop to design, change tracking | Source | **Partial** | Commissioning field file (`.kcfield`), punch list with photos, redlines applied as one undo step. Not yet tied to the build order |
| 22 | Production orders, dashboard, central server | Source | **Skip** | Needs a server; against local-first |
| 23 | Saved 3D views | Source (2026) | **Done** | `view3d_views.rs` |
| 24 | Revisions and associative updates | Source | **Done** | Revisions, compare, change report |
| 25 | EPLAN project import | n/a | **Done in part** | PXF read and layout import (knowledge only) |
| 26 | Cloud / on-premises deployment | Source | **n/a** | Local by design |

### Where KoperCAD is already ahead or different

Own thermal and airflow solver with 3D heat cloud and flow (Pro Panel hands off to RiTherm), live simulation, commissioning
and learning modules, per-user install, printed 1:1 templates with a scale check, bench-first outputs for hand work.
These are *our* claims about KoperCAD; whether Pro Panel lacks equivalents beyond what its public pages list is *inferred*.

### Ranked next lanes

Ranked for ambition and for a hand-building workshop. Effort: S one PR, M two to four, L five or more.

| Rank | Lane | Closes | Effort | Why |
| --- | --- | --- | --- | --- |
| 1 | **Real part data at scale** (finish PARTS-REAL: user-run Rittal, Wago, Phoenix, Schneider, Siemens, Eaton, ABB/Hager import, quality report, 3D and pins) | 3, and unlocks 6, 11, 15 | L | Pro Panel's moat is its Data Portal. Every other number depends on it. Owner-wanted |
| 2 | **Build mode: guided assembly and wiring on a tablet** (interactive checklist in the app: rails, ducts, devices with drill marks, then wire by wire with route and cut length, status per item, "question to design" and change-aware after revisions; reuses `.kcfield` and the commissioning tablet layout) | 19, 20, 21 | L | Biggest gap that matters to hand building; replaces Smart Production without a server. Local, no admin |
| 3 | **Wire production data parity**: bundle numbers, end processing and direction per end, sequence, routing track in the cut list and CSV; ferrule and strip data per wire; hand-friendly cut order by bundle | 13, 20 | M | Cheap, exact data; also the base for any machine format later |
| 4 | **Enclosure and accessory catalog**: enclosures, plates, plinths, partitions, door cut-outs with real sizes and 3D, choose-and-place | 2 | M-L | Needs lane 1's importers; makes cabinet start-up as quick as RiPanel |
| 5 | **Cooling selection**: choose fan, filter or cooling unit from catalog capacity against the heat result; report as "planning value" | 15 | S-M | Completes a chain we already do better than the first half |
| 6 | **Access and collision with real meshes** (tool access, wire bend, door with devices, STEP meshes, not only boxes) | 6, 7 | L | Ambitious, needs part meshes from lane 1; keep honest labels |
| 7 | **Busbar and copper layout** (check `copper-discipline` thread first) | 16 | M | Only if the shop builds distribution boards |
| 8 | **Machine formats** (one wire machine or one drilling machine, from the maker's own spec) | 10, 14 | M each | Wait until the owner names a machine; do not guess |
| Skip | Production orders and dashboard, cloud, AR | 22, 26 | | Against local-first |

### Proposed new threads

1. Build mode (lane 2). 2. Wire production data parity (lane 3). 3. Enclosure and accessory catalog (lane 4, starts after
PARTS-REAL imports exist). 4. Cooling selection (lane 5). Lane 1 continues in the existing parts threads. Lanes 6 to 8 wait.

### Wanted and not wanted (unchanged)

Wanted: real part data, printed bench output, standards report. Not wanted: Dutch menus, symbol editor, label sticker sheet.

### Questions for the owner

Which wire machine or drilling machine, if any, will the shop get? Is the shop going to build busbar systems? Does the shop
want the tablet on the bench (lane 2 shapes the layout)?

### Sources 2026-10-22 (public pages only)

- Pro Panel product page: https://www.eplan.com/us-en/products/eplan-pro-panel/
- Wire production export: https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/productionwiringgui_k_start.htm
- Smart Production: https://www.eplan.com/us-en/products/eplan-smart-production/
- Panel builder workflow: https://www.eplan.com/us-en/blog/software/eplan-panel-builder-blog-02-7-30-2026
- Pro Panel 2026 news: https://frame-engineering.de/neue-version-2026-neuerungen-in-pro-panel/ and https://www.eplan.com/us-en/about-us/press/first-sneak-peek-into-the-upcoming-eplan-platform-2026/

---

# Original analysis, 2026-10-15

Status: draft for the owner, 2026-10-15. Research only, no code. It feeds the
physical-design workstreams (WS-15 layout, WS-16 routing) and the follow-up
threads in section 8. Where it disagrees with `docs/DESIGN.md` or
`docs/design/cabinet-layout.md`, those win and this file is corrected.

## 1. Scope and ground rules

- **Capabilities only.** This lists what Pro Panel does, taken from EPLAN's
  public product pages, help pages and trade press (section 10). It copies no
  EPLAN data, no library and no file format definitions. Part data comes from
  the makers (Wago, Phoenix Contact, Siemens, Rittal and others), never from
  EPLAN's library.
- **Local only, no admin rights.** Nothing here needs a server, a cloud
  account or an installer that elevates. Features that need a maker's login or
  a maker's online service (see "skip") stay out.
- **2D first, 3D later.** A panel builder's questions are answered by a plate
  drawing, a side view and a few numbers long before a rotating 3D model helps.
  The data model gets the depth and the door right first, so a 3D view is a
  drawing task later, not a rewrite.
- **Not a sizing authority.** Thermal and clearance output is a planning aid.
  EPLAN says the same of its own loss calculation, and so do we, in the UI.
- **What I could not confirm.** The public pages I could read do not describe a
  door-swing motion simulation in Pro Panel. They describe doors as mounting
  surfaces and enclosure components, plus "dynamic collision control". Section
  2 marks that row accordingly. The owner may know the real feature better
  than the web does; a screen recording or the menu names would settle it.

## 2. What Pro Panel does

Pro Panel is the 3D control cabinet module of the EPLAN Platform. It starts from
the schematic's device list and ends at the workshop: machining data, wire
lengths and mounting instructions.

| # | Capability | What it does (public description) |
| --- | --- | --- |
| C1 | Enclosure and layout space | A layout space holds the enclosure, mounting plates, doors, walls and profiles in a 3D view, independent of the schematic pages. Enclosures come from maker configurators (Rittal RiPanel) with their 3D models. Items form a hierarchy: moving or deleting a rail moves or deletes the devices on it. |
| C2 | Device placement | The navigator lists every device that has a part; select one or many and place them on a mounting surface. A status mark says whether a placed device still exists in the schematic ("placed, not in project"). Placement uses the maker's real dimensions. |
| C3 | Rails, ducts, accessories | DIN rails, wire ducts, and variable-length parts (cut to length) are placed with their production data entered once in master data. |
| C4 | Mounting surfaces | Devices can mount on the plate, the door, a side wall or a profile, not only the back plate. |
| C5 | Collision and installation checks | "Dynamic collision control" while placing, and a check of mounting clearances (heat, EMC, accessibility). |
| C6 | Door and enclosure motion | Not confirmed from public sources (see section 1). Doors exist as mounting surfaces and parts of the enclosure model. |
| C7 | Holes and cut-outs | Drill holes and millings are derived from the placement. Door cut-outs (display, selector) are part of the model. |
| C8 | Machining output | NC data for plate and enclosure machining (Rittal Automation Systems, Steinhauer, Perforex are named), plus associative mounting drawings and drilling templates. Each machine format is a licence option. |
| C9 | 3D wire routing | Route networks (real or virtual ducts), routing rules, "wiring holes" in plates and partitions for the shortest path, source and destination points, bundling. Wire and cable lengths come from the 3D path, not a flat allowance. |
| C10 | Duct fill | Fill level per duct, re-checked after every routing change. |
| C11 | Wire production data | A wire list with length, colour, cross-section, both ends, end processing and the ducts passed, exported to wire-processing machines (Rittal WT, Komax, Schleuniger, Metzner, Steinhauer, CadCabel; each a licence option). |
| C12 | Thermal design | Total power dissipation of the devices (plus busbars) from per-part loss data, a loss-density map to show hot spots, optional air-conditioned and reserved areas, and a hand-off to Rittal Therm to select a cooling unit. Rules sets check "correct mounting layout". Flagged as a planning value that the enclosure maker must verify. |
| C13 | 2D documents from the 3D model | Model views of plate and enclosure with dimensions, placed on project pages as production documents. |
| C14 | Reports | Enclosure legend, bill of material, cutting list (rails, ducts), parts list, wiring list, labels and ferrules. |
| C15 | Work instructions | Smart Mounting / Smart Production: browser-based, step-by-step assembly and wiring checklists on a tablet, 3D view with dimensions and drill patterns, traffic-light status per component, comments to engineering, change tracking between versions, production orders. |
| C16 | Platform | Revisions, rights, API, associative updates from schematic to layout, cloud or on-premises deployment, Data Portal for part data. |

Two observations that shape our plan:

1. The value is a **chain**: schematic to placement to holes and wire lengths to
   the workshop. Each link is only worth what the previous one gives it. 3D is
   the presentation of that chain, not the chain.
2. The machine-specific outputs (C8, C11) are sold as separate options because
   each needs a vendor's format. They are the expensive tail, not the basics.

## 3. What KoperCAD already has

State of `main` on 2026-10-15 (9caf1cc); open pull requests are named as such.

| Area | What exists | Where |
| --- | --- | --- |
| Enclosure and plate | `Structure` with an enclosure flag; `MountingPlate { width, height, margin }` | `novo-core` (#340, #362) |
| Rails and ducts | `Rail { kind: DinRail or Duct, x, y, length, vertical, width, height }` | `novo-core` (#362) |
| Placement | `CabinetPlacement { device, structure, x, y, rotated, rail, rail_offset }`, one per device | `novo-core` (#340) |
| Footprints and checks | Footprint from part width and height (guessed size flagged), collisions, off-plate, clearance violations (default 10 mm), plate fit, rail overflow | `novo-layout` (#287, #289) |
| Planner | `place all`: sorted by letter code and page order, fills rails left to right, preview, one undo step | `novo-layout` (#320, #322) |
| Canvas | Cabinet scene with collisions and too-close pairs marked | `novo-canvas` (#349) |
| App | Cabinet canvas with Place all (#372, open), drag from the unplaced list, rotate, delete (#374, open), key routing (#376, open) | `novo-app` |
| Routing engine | Duct graph, anchor stubs, shortest path with fill penalty, length with slack, free-air fallback, duct fill and problems; model bridge reads duct rails | `novo-routing` (#334, #358, #371) |
| Part data | `width_mm`, `height_mm`, `depth_mm` keys; STEP bounding box and BMEcat reader (#356, open) | `novo-parts` |
| Output | Vector PDF with pages and title block; DXF reader and writer; CLI `export dxf` | `novo-pdf`, `novo-interop`, `novo-cli` (#344, #370) |
| Reports | BOM, device list, wire list, contents | `novo-reports` |
| Checks | Problems with severity and location; has a cross-section and ambient assumption already for cable ampacity | `novo-checks` |

Known holes the existing design documents already name: terminal positions on a
device footprint are not in the part data, so routing has no wire jobs yet
(`wire-routing.md` 6a); depth is stored but nothing reads it; there is no door,
no mounting surface other than the back plate, and no thermal data.

## 4. Gap matrix

| Capability | Status | Verdict |
| --- | --- | --- |
| C1 Enclosure and layout space | Plate only; no interior size, no depth, no door | **Basic, extend** (R2) |
| C2 Placement | Done in 2D; status mark for "placed, not in project" is the unplaced list | Have |
| C3 Rails and ducts | Stored and routed; cut length is the item length | Have; cutting list **basic** (R6) |
| C4 Mounting surfaces | Back plate only | Later: door and side wall (R5) |
| C5 Collision and clearance | 2D collisions and a flat gap with per-device override | **Basic, extend** with per-part rules (R1) |
| C6 Door and enclosure motion | Nothing | **Basic as a 2D swing check** (R2); motion animation skipped |
| C7 Holes and cut-outs | Nothing | **Basic** (R5) |
| C8 Machining output | Nothing | Generic drawing and table first (R5); machine formats later |
| C9 3D wire routing | 2D duct routing engine, no jobs, no UI | **Basic**: finish 2D, depth as a constant (R4) |
| C10 Duct fill | Engine done | Have; needs UI and a check (R4) |
| C11 Wire production data | Wire list exists without lengths | **Basic**: lengths into the wire list and a generic CSV (R4) |
| C12 Thermal design | Nothing; no loss data on parts | **Basic estimate** (R3) |
| C13 2D documents | PDF and DXF writers exist | **Basic**: plate and side view drawing (R5) |
| C14 Reports | BOM, wire list exist | Cutting list and enclosure legend (R6) |
| C15 Work instructions | Nothing | **Worth doing, later** (R8) |
| C16 Platform | Revisions, undo and one model exist; local-only by design | Have what we need |
| 3D view | Nothing | Later (R9) |

## 5. Data this needs

Everything below comes from maker data or from the user, never from a copied
library. Items already present are marked.

| Field | On | Used by | Source |
| --- | --- | --- | --- |
| `width_mm`, `height_mm`, `depth_mm` | part | footprint, depth check, 3D | Present |
| `power_loss_w` (and an optional `load_factor`) | part | thermal (R3) | Maker datasheet or ETIM feature through the BMEcat reader (#356); typed by hand otherwise |
| `mounting`: DIN rail, screw, plate, door cut-out | part | placement rules, holes (R5) | Maker data; default DIN rail for catalog parts with a rail clip |
| Clearances above, below, left, right | part | clearance rules (R1) | Maker installation notes; letter-code defaults for drives, fuses and contactors |
| Hole or cut-out pattern | part | holes (R5) | STEP file or typed; a rail needs none, a door display needs a cut-out size |
| Connection point positions on the footprint | part | routing jobs (R4) | STEP, part symbol, or typed; already flagged as open in `wire-routing.md` |
| Interior width, height, depth; plate setback; door hinge side and opening angle | enclosure `Structure` | depth check, door swing (R2), thermal surface (R3) | User input at first, enclosure catalog later |
| Enclosure material and installation (free standing, wall, row) | enclosure | thermal (R3) | User input |
| STEP bounding box | part | depth when `depth_mm` is missing | #356 |
| Full 3D mesh | part | 3D view (R9) | STEP, later, optional |

`novo-core` changes (enclosure interior and door, per-part properties are free-form
property keys so parts need none) are WS-02's to make; this document only names them.

## 6. Ranked plan

Effort: **S** one pull request, **M** two to four, **L** five or more. All
engines are pure functions in `novo-layout`, `novo-routing`, `novo-checks` or
`novo-reports` first, with thin UI after, like the rest of the program.

| Rank | Item | Effort | Depends on | Value |
| --- | --- | --- | --- | --- |
| R1 | **Per-part clearance rules.** Rule sets (default gap, per letter code, per part key) replace the one flat gap; violations say which rule. Adds hot-device spacing that R3 also reads. | S | part keys (data section) | Replaces what designers keep in their heads |
| R2 | **Enclosure interior, depth and door.** Interior size and plate setback on the enclosure. Checks: device depth against available depth, devices that block the door at its swing, a top view with the swing arc. This is the 2D answer to "door simulation". | M | WS-02 enclosure fields; `depth_mm` (present) | High; catches the "does not close" mistakes |
| R3 | **Heat estimate.** Total loss from per-part `power_loss_w` with a load factor, enclosure effective surface from its dimensions and installation, steady-state interior temperature rise by natural convection, required cooling output, and a loss-density map on the Cabinet canvas. Output as a Problem in `novo-checks` ("needs about N W of cooling, or a fan of about V m3/h"). Plain steel surface constant; other materials by user input. Carries the "planning value, verify with the maker" note. | M | `power_loss_w` on parts (R0 below); enclosure fields from R2 | High; the most asked-for number after "does it fit" |
| R4 | **Routing in the app and wire lengths.** Wire jobs from the netlist and part connection points, route all with preview, lengths into the wire list and a generic wire-cut CSV, duct fill as a check. Depth enters as a fixed vertical allowance per end. | L | connection points on parts; wire list (WS-13); routing engine (done) | High; ends hand-typed lengths |
| R5 | **Plate drawing, drill list and cut-outs.** A dimensioned mounting plate drawing (PDF through `novo-pdf`, DXF through `novo-interop`) and a drill and cut-out table from placements. Door cut-outs from part patterns. Generic output only. | M | mounting and hole pattern keys; DXF writer (done) | High for the workshop; first step toward machine data |
| R6 | **Cutting list and enclosure legend.** Rails and ducts by length and count, enclosure items with plate size, as report pages. | S | placements (done) | Medium, cheap |
| R7 | **Rules-based place all.** Power devices high, drives with their clearance, signal and power ducts separated, hot devices away from the top. Reads R1 and R3. | M | R1, R3 | Medium; today's planner is first-fit |
| R8 | **Assembly steps.** An ordered work instruction (rails, ducts, devices, with positions) as a PDF checklist from the placements. A local analogue of EPLAN's Smart Mounting without its server. | M | R5 | Medium; workshop value |
| R9 | **3D view.** Orbit view on `wgpu`: plate, rails, ducts and devices as extruded boxes from footprint and depth. Door open and closed. STEP meshes are a separate, much later decision (see section 7). | M for boxes | R2 | Presentation, not analysis |

**R0, the data step before R3 and R5.** Add the part keys in section 5 to
`novo-parts` (`power_loss_w`, `mounting`, clearances), read them in the BMEcat
reader where a maker supplies them, and show them in the part inspector and the
part browser. Small, and everything above waits on it. It is owned by the Parts
library thread and should start now.

Suggested order: R0, R1 and R2 in parallel (different crates), then R3 and R6,
then R4 and R5 in parallel, then R7, R8, R9.

## 7. What to skip, and why

- **Machine formats for Rittal Perforex, Steinhauer, Komax, Schleuniger, Metzner,
  CadCabel and Rittal WT.** EPLAN sells each as a licence option; the formats
  are the machine makers'. Revisit when the owner names the machine he uses,
  then write that one exporter from the maker's own specification. The generic
  drill table and wire-cut CSV (R4, R5) cover the first workshop.
- **Maker online services: Rittal RiPanel and RiTherm.** They need a login and
  a network, which breaks local-first, and (for Therm) the result is the maker's
  sizing, which we should not imitate as authoritative. R3 gives the estimate;
  the user can still copy the loss total into the maker's tool.
- **Full 3D collision with real meshes, and dynamic door motion.** A 2D footprint
  plus depth plus a swing arc catches nearly every real mistake. Mesh-level
  collision needs a geometry kernel and STEP tessellation, which means a heavy
  native dependency and a larger no-admin build story.
- **STEP mesh import for rendering.** Maybe after R9. Bounding boxes first;
  the `step_box` in #356 already gives them.
- **Copper busbar design, 19-inch frame systems, fluid power layout,
  Collaboration and cloud deployment, augmented reality.** Either a separate
  product-sized effort or against the local-first rule.
- **Production order management, dashboards, tablet server.** The part of
  Smart Production that needs a server. R8 keeps the useful core as a PDF.
- **EMC simulation.** Clearance rules in R1 and R7 cover the part a designer
  acts on.

## 8. Proposed follow-up threads

Each is a small thread with its own branch, in the usual one-crate-per-pull-request
style. Names are suggestions.

1. **Part data for panel building** (Parts library): R0.
2. **Layout rules** (Cabinet layout, WS-15): R1, then R7.
3. **Enclosure, depth and door** (Cabinet layout with WS-02): R2, then R9's data needs.
4. **Heat estimate** (new, `novo-checks` plus a small engine in `novo-layout`): R3.
5. **Routing in the app** (Wire routing, WS-16): R4.
6. **Plate drawing and machining tables** (Outputs plus Interop): R5, R6, later R8.
7. **3D view** (later, after R2 lands): R9.

The owner's decision points: whether threads 4 and 6 start now (recommended:
yes, they only need R0's keys), and which machines the workshop actually uses,
which decides the first real machine format.

## 9. Risks

- Per-part data is the real bottleneck. Without `power_loss_w` and connection
  points a thermal number or a wire length is a guess. The plan flags guessed
  values the way footprints already are, and never shows a guess as a result.
- Thermal estimates are easy to over-trust. The UI wording and the check text say
  "planning value", and the formula and constants live in one tested function with
  the source named.
- Scope drift towards "clone Pro Panel". The skip list is the guard: if a
  proposal does not shorten the path from schematic to workshop on a plain
  Windows machine, it waits.

## 10. Sources

EPLAN public pages, read 2026-10-15, no EPLAN files or data were copied:

- Eplan Pro Panel product page: https://www.eplan.com/us-en/products/eplan-pro-panel/
- 3D mounting layout, topic overview: https://www.eplan.help/en-us/Infoportal/Content/Plattform/2025/Content/htm/cabinetgui_k_start.htm
- 3D mounting layout, principle: https://www.eplan.help/en-us/Infoportal/Content/Plattform/2025/Content/htm/cabinetgui_k_prinzip.htm
- Thermal design integration: https://www.eplan.help/en-us/Infoportal/Content/Plattform/2025/Content/htm/cabinetgui_k_thermischeauslegung.htm
- Total power dissipation: https://eplan.help/en-us/Infoportal/Content/Plattform/2.8/Content/htm/cabinetgui_k_verlustleistung.htm
- Wire production export: https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/productionwiringgui_k_start.htm
- Pro Panel 2.1 release: https://www.automation.com/en-us/products/product12/eplan-releases-pro-panel-21
- Pro Panel 3D addition: https://www.automation.com/article/eplan-adds-3d-to-pro-panel
- Panel production time: https://www.eplan.com/us-en/blog/software/eplan-panel-builder-blog-02-7-30-2026
- Wire scrap: https://www.eplan.com/us-en/blog/software/reducing-wire-scrap-and-trial-and-error-with-eplan-pro-panel
- Smart Mounting: https://eplan-software.com/solutions/eplan-smart-mounting
- 3D mounting layouts (Engineering Center): https://www.controldesign.com/vendors/products/2014/eplan-adds-3d-mounting-panel-layouts-to-engineering-center-efficiencies

Heat balance reference (the surface emission term and the sheet-steel constant
of about 5.5 W/m2K are quoted there; the effective-area method is IEC 60890):

- Rittal climate control handbook, page 6: https://www.rittal.de/downloads/eBook/TSH/EN/Climate_control/pubData/SEO/Page_6.html

The thermal formulas for R3 must be checked against IEC 60890 and a maker
handbook by the implementing thread before the numbers are shown.
