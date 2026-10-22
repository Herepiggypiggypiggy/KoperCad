# EMC visualisation: roadmap

Scope: how EMC shows in KoperCAD (3D cabinet, plate, schematic). Reads findings only through the `Picture` interface; the rule engine thread owns `novo-emc`.

Done
- Concepts (corridors and layers chosen), `Picture` interface, overlays, exposure map, stub, 3D EMC view, panel, legend, EMC example, QA scenario 43-3d-emc.

Next, one PR each
1. 2D mirror on the cabinet plate (hook near `cabinet_routes::draw` in cabinet.rs).
2. Schematic overlay of net classes.
3. Loops, bonding and shields from real data; per-cable EMC card.
4. Adapter from `novo-emc` replacing the stub; fix buttons wired to commands.

Open: hot spot badges overlap when two sit close; frame the camera on the cabinet when the view opens.
