# UI coverage: cabinet, 3D view and simulation

Audit of 2026-10-16 on main at 61c16b0. Question: is every engine capability
reachable from the app, and how deep? "Clicks" counts from the workspace named.
Verdict: OK, too deep, hidden, MISSING (engine only), or FIXED in this PR.

## Cabinet

| Capability | Engine | UI entry | Clicks | Verdict |
| --- | --- | --- | --- | --- |
| Create cabinet, pick panel | `novo-layout` | Cabinet > Panel group (combo, "Create cabinet", "Set plate 600 x 800 mm") | 1 | OK |
| Mounting plate size, margin, fill | `novo-layout::model` | Cabinet > Arrange > Plate setup (also Cabinet > More when narrow) | 1 to 2 | OK |
| Place all by rules, preview before storing | `novo-layout` placement | Cabinet > Place > Place all (`cabinet.place-all`) | 1 | OK |
| Placement rules | `cabinet_rules` | Cabinet > Place > Rules | 1 | OK |
| Move, rotate (R), delete devices | canvas + `novo-layout::state` | Drag on the plate; R / Delete | 0 | OK |
| Enclosure sizing (IEC 60890 estimate) | `novo-layout::sizing` | Cabinet > Arrange > Size enclosure; command `cabinet.size-enclosure` | 1 | OK |
| Door and hinges, inside size, depth checks | `novo-layout::enclosure` | Cabinet inspector, "Inside and door" (needs the plate card or a selection) | 2 | too deep: not on the bar |
| Rails and ducts (add, move, resize, delete) | `novo-layout::state` | Cabinet > Route > Rails and ducts (Ctrl+Shift+R) | 1 | OK |
| Route all wires through ducts, fill | `novo-routing` | Cabinet > Route > Route all | 1 | OK |
| Cut list | `novo-layout::stock` | Cabinet > Route > Cut list | 1 | OK |
| Clash checks (collision, outside, too close, rail overflow, door) | `novo-layout::check`, `collide` | Problems menu on the Cabinet bar (count in red) and the inspector | 1 | OK |
| Table view of devices | `cabinet_table` | Cabinet > View > Table (Ctrl+T) | 1 | OK |
| Plate drawing PDF with drill table | `novo-pdf` | Cabinet > Export | 2 | OK |

## Heat and simulation

| Capability | Engine | UI entry | Clicks | Verdict |
| --- | --- | --- | --- | --- |
| Heat layer, synthetic estimate (no run) | `novo-heatview` | Cabinet > View > Heat (Shift+H) | 1 | OK, but labelled only "Heat"; the estimate is not a simulation |
| Simulate (Coarse, Medium, Fine ladder) | `novo-cfd::sched`, `CabinetSolver` | Cabinet > Heat strip > Simulate; Ctrl+K "heat" | 2 | OK in Cabinet |
| Simulate from the 3D view | same | none before this PR; `heat.simulate` threw the user into the Cabinet | n/a | FIXED: 3D > Heat panel (top right) > Simulate; the command now stays in 3D |
| Run states: not run, running with progress, done, out of date | `heat_progress`, `heat_run` | Cabinet heat strip (bar, sparkline, Cancel); 3D Heat panel | 0 | FIXED in 3D (plain sentence for each state) |
| Quality level | `Plan::up_to(top)` | was hard-wired to Fine | n/a | FIXED: 3D Heat panel > Settings > Quality (Quick, Balanced, Thorough) |
| Turbulence model (laminar, k-omega SST, k-epsilon) | `CabinetSolver::with_turbulence` | was hard-wired to Laminar | n/a | FIXED: Settings > Turbulence. Default stays Laminar |
| Vents (grilles, louvres) | `novo_thermal::Vent`, `novo-cfd::openings` | none; the bridge builds a sealed enclosure | n/a | FIXED in part: Settings > Vents (session only, not saved in the project, needs a model field from WS-02) |
| Fans | `novo_thermal::Fan`, `novo-cfd::openings` | none | n/a | MISSING: same gap as vents; follow-up |
| Solids, rails and ducts as obstacles | `novo-cfd::solids`, bridge | automatic from the cabinet data | 0 | OK (no control needed) |
| Slice, shell (iso surface), airflow streamlines and tracers, device tint | `novo-heatview::cfd`, `heat_3d` | 3D "Heat" strip above the view, shown only once a result exists | 1 after a run | hidden before a run: now explained in the panel |
| Contours, hot spots, badges, airflow on the plate | `novo-heatview` | Cabinet heat strip chips | 1 | OK |
| Compare two runs (Keep as A, delta, wipe, side by side) | `heat_compare` | Cabinet heat strip; also shown in the 3D panel next to Simulate | 1 | OK |
| Accuracy statement, "not validated", limits, what the bridge assumed | `novo-cfd::accuracy`, `BridgeReport` | status label next to Simulate; legend; 3D panel > What is this? | 1 | FIXED in 3D (full text and limits). Label stays until validation supports it |
| Heat-up over time (transient) | `novo_cfd::transient`, `novo_thermal` | none | n/a | MISSING: needs a UI (time slider and curve) |
| Radiation on or off | `novo-cfd::rad` | none; statement says it is not validated | n/a | MISSING: expose once validated |
| Results export (field data, CSV, report page) | none found in `novo-cfd` or `novo-heatview` | none | n/a | MISSING: engine and UI |
| Scheduler, cancel, progress | `novo-cfd::sched` | Cancel button and bar in both views | 1 | OK |
| Headless run (CLI or Python) | `novo-cli` has no thermal command | none | n/a | MISSING |

## 3D view

| Capability | UI entry | Verdict |
| --- | --- | --- |
| Open the 3D view | Top bar segment "3D"; `view.3d` | OK |
| Layers: enclosure, door, wires | 3D toolbar top left; `view.3d_shell`, `view.3d_door`, `view.3d_wires` | OK |
| Door open and close, angle | Toolbar button and slider; Shift+D | OK |
| Move devices, rails, ducts in 3D | Toolbar "Move" | OK |
| Shadows | Toolbar "Shadows"; `view.3d_shadows` | OK |
| Camera presets iso, front, top | commands `view.iso`, `view.front`, `view.top` only | hidden: no toolbar buttons |
| Frame statistics | `view.3d_stats` only | hidden by design (diagnostic) |
| Selection sync with the Cabinet | automatic | OK |

## What this PR changes

3D > Heat panel (top right of the 3D view): Simulate or Cancel, progress,
a plain sentence for each state, Settings for the next run (quality,
turbulence, vents), and "What is this?" with the accuracy statement. The
`heat.simulate` command (Ctrl+K "heat") no longer leaves the 3D view.
