# Vents and airflow in 3D heat: roadmap

Scope: Tadjiro found vent placement arbitrary, vents invisible in 3D and the airflow hard to see.

## Done (PR 1, branch claude/project-thread-827buu)
- Openings model in `heat_settings.rs`: grille, fan, filter fan; walls incl. the door; centre in mm;
  every opening is fitted inside its wall (the solver used to clip an opening hanging over an edge,
  silently shrinking it); presets (low inlet grille, high outlet grille, filter fan low on the door,
  exhaust grille high on the door, exhaust fan in the top); undo/redo with one step per drag.
- Fans reach the solver (`Scenario::fans`); before only grilles could be added.
- `heat_openings.rs`: frame, slats or rotor, in/out arrow and a label per opening in 3D, before any run;
  flow through each opening read from the result (model value).
- `view3d_vents.rs`: Vents and fans panel (left of the 3D view when Heat is open).
- Airflow: streamlines coloured by speed with arrowheads, a m/s colour bar in the Heat strip,
  streamlines seeded at the inlets, Airflow switched on once for the first result with openings.
- Fixes found on the way: pinned hot-spot labels only painted in Measure mode; Heat strip unreadable in
  the light look (no fill over the black canvas clear).

## Next
1. Save openings in the project (needs a WS-02 model change: ask WS-02; until then session only).
2. Drag an opening on its wall in 3D (pick ray against the wall plane, `novo_view3d::drag_plane`).
3. Openings in the Cabinet (2D) heat view and the Cooling selection (#1342): place the proposed fan.
4. Filter mat pressure drop as its own term, fan curves from part data (`airflow_m3h`).
5. Mass balance note when in and out flows of the openings differ by more than a few per cent.
