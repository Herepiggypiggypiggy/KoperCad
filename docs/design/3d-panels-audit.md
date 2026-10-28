# 3D and Cabinet panels, layers and toggles: audit and the one-state rule

Thread "Rework 3D view panels and layer toggles", 2026-10-28. The owner's complaints: EMC cannot be turned off in the 3D view; the 3D configuration panel is frustrating; Inspector, Temperature, Door, Sizing and the others are separate popups that are hard to find, cannot be closed and are hard to open again; buttons are doubled (heat off in the 3D panel stays on elsewhere). Design target: `visual-redesign-3d.md` (side panel of foldable sections, rail lanes V3a/V3b, window rules lane V1). This page is the audit that the first PR (the option registry) answers.

Evidence: harness scenario `qa/scenarios/1599-3d-panels-audit.qa` at 1366x768 and 1920x1080, and the code.

## 1. Causes found in the code

| Complaint | Cause | Fixed by |
| --- | --- | --- |
| EMC cannot be turned off in 3D | `view3d.rs` set `emc.on = controls.emc` before the toolbar and wrote `controls.emc = emc.on` back after the panels. The toolbar click set `controls.emc = false`, the write-back set it to true again in the same frame | One owner (`view3d.emc.on`); the toolbar writes it through the registry; no copy |
| Heat "stays on" after it is switched off in the 3D toolbar | The toolbar's Heat button only flipped `controls.heat_panel`, which opened the Heat and Vents panels. The strip above the view, the slice/shell/airflow overlays, the pinned labels and the opening markers followed whether a result existed, not the button. The Cabinet has its own `heat.on` and the Heat command switched to the Cabinet | One shared `heat.on` for the Cabinet and 3D; off hides strip, overlays, labels, markers and both panels |
| Panels pile on each other | Heat, EMC and Wire runs all start in the right slot under the view cube (Heat panel hidden behind the EMC panel in the screenshots); Vents and fans and the Inspector share the left slot | Side panel of sections (next PR) |
| Panels cannot be closed or found | The panel helper (`panel.rs`) draws a window with a collapse arrow and no close button; closing is only possible by turning the owning switch off, and Inspector is `fixed_open` | Section header with fold, undock and close; Sections menu; Esc (next PR) |
| The Layers popover is confusing | It mixed visibility layers, two walk-through modes (Build order, Wire-it) and a preference (Trackpad pans), and shows "Layers (7)" counting some of them | Registry homes: Layer, Preference, Walk (own heading), Overlay and Mode on the toolbar |

## 2. Every 3D switch, its owner and where it is reached

State is now one `ViewOption` in `crates/novo-app/src/app/view3d_options.rs`. The toolbar edits a snapshot and the host writes the difference to the owner; commands call the same writer; menu rows read the same value (drawn selected).

| Option | Owner of the value | Toolbar | Command (Ctrl K) |
| --- | --- | --- | --- |
| Enclosure, Door, Wires, Device shapes, Slotted ducts | `controls.layers` | Layers popover | `view.3d_shell`, `view.3d_door_layer`*, `view.3d_wires`, `view.3d_device_shapes`*, `view.3d_slotted_ducts` |
| Surfaces, Duct fill, Connection points | `controls` | Layers popover | `view.3d_surfaces`*, `view.3d_duct_fill`*, `view.3d_pins`* |
| Shadows, Metal finish, Occlusion, Missing data | `controls` | Layers popover | `view.3d_shadows`, `view.3d_finish`, `view.3d_ssao`, `view.3d_missing_data`* |
| Dimensions (measuring aid, next to Measure) | `controls.dimensions` | Dimensions | `view.3d_dimensions` |
| Trackpad pans (preference) | `settings.trackpad_pans` | Layers popover | `view.3d_trackpad_pans`* |
| Build order, Wire-it (walk through) | `controls.assembly`, `controls.wire_it` | Layers popover, "Walk through" | `view.3d_assembly`, `view.3d_wire_it` |
| Heat | `cabinet.heat.on` (shared with the Cabinet) | Heat | `heat.toggle` (in 3D it flips the switch in place; elsewhere it opens the Cabinet heat view) |
| EMC | `view3d.emc.on` | EMC | `view.emc` |
| Wire runs | `view3d.wire_runs` | none yet (side panel) | `view.3d_wire_runs` |
| Move, Measure, Section | `controls.move_items`, `measure.on`, `view.state.section` | Move, Measure, Section | `view.3d_move`*, `view.3d_measure`, `view.3d_section` |

\* new palette-only commands, so every option has a Ctrl K entry.

Not options (panels and cards, handled by the side panel PR): Heat panel, Vents and fans, EMC panel and legend, Wire runs card, Inspector (with its door and sizing sections), rail and duct card, Place list, Position fields, Statistics, Hints, Heat strip, Build order strip, Wire-it strip, Cooling advice, Cooling what-if.

## 3. Cabinet workspace (not changed by the first PR)

| Control | State | Note |
| --- | --- | --- |
| Heat (bar or "More" menu, never both: the toolbar folds it) | `cabinet.heat.on` | Same switch as 3D heat since this PR |
| EMC (bar or "More") | `cabinet.emc.on` | A second EMC state, separate from `view3d.emc.on`; unify in the Cabinet PR with its layers |
| Route all | `cabinet.routes` | |
| Inspector, Rules, Rails and ducts, Cut list, Sizing | their own windows | Cabinet rail lane V3c |

## 4. Rules

1. One state, one owner. A surface reads the value and asks to change it; it keeps no copy.
2. A toggle appears in one place on the toolbar side: overlays and modes on the toolbar, layers and preferences in the Layers popover.
3. Off means off: a switch that is off hides its drawing, its legend and its panels, and no frame turns it back on. Tests: `view3d_options::tests`.
4. Not verified on Windows.
