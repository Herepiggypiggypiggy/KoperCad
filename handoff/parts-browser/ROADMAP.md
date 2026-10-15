# Parts browser and Import (app side)

Scope: the Parts window in `novo-app` over the `novo-parts` engine (docs/PARTS-LIBRARY-DESIGN.md).

1. Browser window: search, maker filter, detail (dimensions, power loss, clearances, connections), In library / Import per row. (PR 1)
2. Import flow: Open page, Downloads watch, drop a file, progress and result, merge into the user catalog, source record shown. (PR 2)
3. Hand 2D/3D members to novo-interop and the 3D part geometry thread. (later)

Coordinate: Parts library thread (engine, source table), Part power loss and clearances thread (PR #419 keys), 3D part geometry thread.
