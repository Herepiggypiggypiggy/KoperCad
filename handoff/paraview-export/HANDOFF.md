# ParaView export (VTK) handoff

Done: `novo-heatview::vtk` writes a solved cabinet as `.vtr` (rectilinear grid, cell data, binary base64), a heat-up as one `.vtr` per frame plus a `.pvd`, and a small `pvpython` script. `app/heat_export.rs` has the commands `heat.export_vtk` and `heat.open_paraview` (Export centre > Exchange, command search, and the "ParaView" button in the Heat panel once a result exists). ParaView is found via `KOPERCAD_PARAVIEW`, then `%LOCALAPPDATA%`, the profile and Downloads, then Program Files. Nothing of ParaView/VTK is bundled or copied.

Frame: x right, y UP, z plate to door (same as the 3D view). Solver y is down, so y edges are mirrored, rows reversed, velocity y negated. Unit mm (field data `LengthUnitInMetres`), a metres option exists in the writer but has no UI yet.

Arrays: Temperature (degC), Velocity (m/s), Pressure (Pa), TurbulentViscosityRatio, Material, Solid, WallHeatFlux. Field data carries the Statement (model, not validated unless the snapshot says so), iterations, convergence, energy balance, time.

NOT yet verified: nobody has opened a file in a real ParaView. Owner must do that before we say "works".

Not done: EMC map as thin `.vtr`, 2D plate results, metres option in the UI, importing VTK back (#9 of the research summary).
