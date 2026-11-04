# Evidence: Fans inside section (Heat H8 UI)

Head `a11a2294fd`. Reports in `/mnt/project-files/qa/h8-final/<scenario>/report.md`; each says "source at git a11a2294fd".

## Where the user finds it
3D view, Heat layer, **Vents and fans** panel, bottom, **Fans inside** (add fan inside, direction, from the left, height above floor, off the plate, diameter, flow and pressure, installed %). Saved with the project (structure property `heat.inside_fans`), used by the next 2D run. The 3D solver ignores these fans for now.

## Labels (visible, not hover-only)
"(model)" beside the section title and in each fan row. With a fan present: "Model, not measurement. Flow and pressure are the maker's free-air values; Installed lowers them. A new fan starts with assumed values." Longer explanation behind the "?".

## New scenarios (1860 at 1366x768, 1860b at 1920x1080)
Add a fan inside, row and fields shown, check_layout (no clipped fields), remove it. Both pass. The panel is docked; floating was not run. At 1366 the section is below the visible part of the panel, so the scenario scrolls the panel first (the panel scrolls; the content is not clipped).

## Existing Vents and Heat scenarios at both sizes
| Group | Result |
| --- | --- |
| 1814, 200, 201, 202, 231, 232, 240, 250 (heat strip), 250 (heat scale) | pass |
| 1815 | red at one size: "Preliminary is on screen"; red on main too (main-red-0311 shards, source daa4147941: F-065/F-066) |
| 380 | red: "click_text Quick" / "Model comparison"; red on main too (main-red-0311-s1 F-104/F-105, qafix-heat-final F-021) |
| 18150, 230 | one "click_text Quick" failure at 1366 on the first run, with the button visible in the screenshot; passes on re-run (see `rerun-*`) |

My own regression found and fixed on the way: with the section above the suggestion card it pushed the "Through-flow" line out of the 1366 panel and 1814 failed; the section now sits at the bottom and is one line when empty.

## Not tested
Real Windows. A heat run with a fan inside against a measurement (nothing is measured). Floating panel. The 3D solver with these fans (not built).
