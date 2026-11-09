# Heat flow lines handoff
Where: 3D view > Heat strip > Heat flow = "Carried by air" or "Air and solids" shows two sliders, Lines (how many) and Intensity (thickness and brightness). Hover text explains each.
Code: `LineView` in `heat_carry.rs`, `Heat3d::carry_lines`, `stream_lines_styled` / `tracers_styled` in `heat_3d.rs`, controls in `view3d.rs`.
Before, heat lines borrowed the airflow Strength slider, which is hidden while Flow is Off, so they had no control. Defaults reproduce the old look (width 1.8, floor 60, dash 3.6, number 0.25).
Scenario: `qa/scenarios/18151-3d-heat-flow-lines-number-and-intensity.qa`.
