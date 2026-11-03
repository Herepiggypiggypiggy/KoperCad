# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 22 run(s): 22 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/kc-target2/debug/kopercad`, built 1793727124 (unix time), source at git d8015d8295.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Build mode: first step shows the design's open Issues next to the layout verdict (1610-build-banner-issues) | Cabinet | 1366x768 | pass | - | 2 |
| Build mode: first step shows the design's open Issues next to the layout verdict (1610-build-banner-issues) | Cabinet | 1920x1080 | pass | - | 2 |
| A macro placed past the sheet edge is reported (16164-macro-off-sheet-warning) | Schematic | 1366x768 | pass | - | 1 |
| A device tag over 60 characters is refused and the box keeps focus (16165-rename-tag-too-long) | Schematic | 1366x768 | pass | - | 2 |
| Cabinet plate: EMC findings the engine gives no place (motor cable shield, pigtail) are marked at their drive and pigtail end (300-emc-unplaced-findings-marked) | Cabinet | 1366x768 | pass | - | 5 |
| Cabinet plate (1920x1080): EMC findings the engine gives no place (motor cable shield, pigtail) are marked at their drive and pigtail end (301-emc-unplaced-findings-marked-1920) | Cabinet | 1920x1080 | pass | - | 5 |
| Three-phase supply is fed when placed after the starter (310-supply-macro-feeds-live) | Live | 1366x768 | pass | - | 4 |
| Three-phase supply is fed when placed after the starter (310-supply-macro-feeds-live) | Live | 1920x1080 | pass | - | 4 |
| Wire runs panel keeps clear of the Inspector and the other 3D panels (32-wire-runs-panel) | 3D | 1366x768 | pass | - | 4 |
| Wire runs panel keeps clear of the Inspector and the other 3D panels (32-wire-runs-panel) | 3D | 1920x1080 | pass | - | 4 |
| Run card: bend radius and heat neighbour checks (models), read at the foot of the card (33-run-card-checks) | 3D | 1366x768 | pass | - | 2 |
| Run card: bend radius and heat neighbour checks (models), read at the foot of the card (33-run-card-checks) | 3D | 1920x1080 | pass | - | 2 |
| 3D view: metal finish on the plate and rails, and the switch back to flat colours (1920x1080) (341-3d-metal-finish-1920) | 3D | 1920x1080 | pass | - | 5 |
| 3D view: metal finish on the plate and rails, and the switch back to flat colours (1366x768) (341-3d-metal-finish) | 3D | 1366x768 | pass | - | 5 |
| 3D view: first-time walk (open, navigate, layers, dimensions, clearance, section, door, saved views) (40-3d-walk) | 3D | 1366x768 | pass | - | 7 |
| 3D view: first-time walk (open, navigate, layers, dimensions, clearance, section, door, saved views) (40-3d-walk) | 3D | 1920x1080 | pass | - | 7 |
| 3D view: section plane and hide / show all (46-3d-section-hide) | 3D | 1366x768 | pass | - | 3 |
| 3D view: section plane and hide / show all (46-3d-section-hide) | 3D | 1920x1080 | pass | - | 3 |
| 3D view: a maker model that came lying down is turned and the turn is kept (52-3d-model-turn) | 3D | 1366x768 | pass | - | 3 |
| 3D view: ducts are drawn slotted, zoomed to one duct, and the Layers toggle makes them solid (58-3d-slotted-ducts) | 3D | 1366x768 | pass | - | 5 |
| 3D view: ducts are drawn slotted, zoomed to one duct, and the Layers toggle makes them solid (59-3d-slotted-ducts-1920) | 3D | 1920x1080 | pass | - | 5 |
| Terminals drawer: the toolbar is one row at 1920x1080 too (9203-terminals-toolbar-one-row-1920) | Terminals | 1920x1080 | pass | - | 5 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
