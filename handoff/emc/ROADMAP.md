# EMC: roadmap

Scope: EMC visualisation and EMC-driven design in KoperCAD. Plan: `docs/EMC.md` (read it first). Never certifies compliance.

## Lanes
| Lane | Thread | Owns |
| --- | --- | --- |
| Lead | EMC plan and scope | docs/EMC.md, this folder, lesson track, manual page, validation doc, honesty wording |
| Rule engine | (sibling, to be started) | `novo-emc`: classification, indices, estimates, rules EMC-01..18, findings into Checks and reports |
| Visualisation | (sibling, to be started) | `novo-view3d` / `novo-app`: zone, class, coupling, loop, bonding views, EMC card, 2D mirror |
| Field (later) | (third thread after E2) | Level 3 quasi-static field, slice through `novo-heatview` |

## Phases
- E0 plan merged, `novo-emc` registered by the coordinator (own small PR).
- E1 classes + shield/bonding facts (Properties first), rules EMC-01..06, Zone/Class views.
- E2 indices (parallel length, distance, crossing, loop area), EMC-07..12, coupling ribbons, loop sheets, card, report page.
- E3 analytic estimates with ranges, bonding graph, part EMC data, drive macro.
- E4 quasi-static field slice, what-if, router EMC cost, Autopilot fixes.
- E5 lessons, manual, handover record, standards-report section.

## Open questions
- Stored fields: Properties first or typed `novo-core` fields (ask WS-02)?
- Which part makers' EMC data can be imported (Parts library thread)?
- Default thresholds (parallel length, distance, loop area): house values or common practice; label either way.
