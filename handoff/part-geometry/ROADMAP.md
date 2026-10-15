# Roadmap: part geometry (3D view)

Scope: STEP to render mesh, cache, box fallback, instancing. Design: docs/design/part-geometry.md.

Done: parser, B-rep, tessellation, decimation, LOD chain, cache, instancing, service, example CLI, tests.

Next, in order:
1. Review of the first draft PR; move into `novo-view3d` once the coordinator adds the crate.
2. Wire to the parts library: `content_id` = SourceRecord sha256, size from catalog.
3. B-spline surface tessellation; spindle torus fix.
4. Per-part rotation override; generated shapes (DIN rail, duct).

Open: final crate home; whether the renderer wants edges per LOD.
