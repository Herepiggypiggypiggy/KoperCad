# Cooling advice and vent sweep: roadmap

Scope: Tadjiro asked (CV-200 heat findings, `audit/real-machine/heat-layout-findings.md`) for a
cooling advice card and a vent and fan position sweep that ranks tries by predicted device temperatures.

## Done (PR 1, branch claude/project-thread-phf292)
- Engine `novo_thermal::sweep`: four kinds of pair (fan in + fan out, vent + vent, vent in + fan out,
  fan in + vent out) over a grid of wall places (door low/high, floor, roof, side walls, plus the x of
  the device worst off), then sizes (fan flow ladder, vent area ladder), then the best of each kind on a
  finer grid next to a baseline on the same grid. Ranked by devices over, tightest margin (half-kelvin
  steps, capped at +2 K so the cheaper option wins once everything is comfortable), cost, hottest air.
- Gains under 1 K are flagged "within the model's noise" (the 2D solver wobbles that much).
- Window `heat_sweep.rs`: Heat strip button "Cooling advice" (Cabinet and 3D), Ctrl+K "Cooling advice
  and vent sweep", plate right-click. Best few with before/after, flow the openings really pass, housing
  temperatures of the worst-off devices. "Add these two to Vents and fans" is one undo step in the list.

## Next
1. Hook the Cooling selection engine (#1342): real fan parts and the person's cooling.csv as the
   ladder (`Settings::fan_flows_m3h`), and propose a cooling unit when no pair works.
2. Draw the winning pair on the plate/door before adding (preview ghost).
3. 3D check of the winner in one click (Simulate with the pair), when the large-cabinet CFD lane lands.
4. Sweep device positions together with openings (with the Cooler layout, #1365).
