# Part geometry for the 3D cabinet view

Status: first implementation in `crates/novo-interop/src/part3d` (draft PR). Owner thread: "3D view: part geometry import".

## Scope

A pure-Rust path from a maker STEP file that the user imported through the parts library importer to a small render mesh, cached per user on the device. Maker files are never bundled and never committed; test fixtures are synthetic. When no usable model exists the result is a plain box from the catalog width, height and depth.

## Pipeline

1. `parse`: lazy STEP (ISO 10303-21) reader. Entities are parsed on demand; complex entities and the unit from `GLOBAL_UNIT_ASSIGNED_CONTEXT` are handled (mm, inch, m).
2. `brep`: reads faces (`ADVANCED_FACE`, loops, `EDGE_CURVE`, lines, circles, ellipses, B-spline curves sampled) on plane, cylinder, cone, sphere and torus surfaces. Other surfaces are flattened and counted as approximated.
3. `tess`: faces are triangulated in parameter space (earcut), refined by sag (chord tolerance 0.05 mm default), with a strip zipper for seamless two-loop cylinders and cones. Orientation comes from the loop direction vote, not from `same_sense`.
4. `build`: walks solids and assemblies (`MAPPED_ITEM`, `REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION`) and AP242 `TRIANGULATED_FACE`. Limits: 96 MiB input, 150 000 faces, 5 000 placements, a deadline and a cancel flag.
5. `decimate`: weld, drop degenerate and duplicate triangles, quadric edge collapse with a boundary penalty and flip checks, vertex clustering as the fallback that guarantees the budget.
6. `mesh`: part-space transform, smoothed normals (40 degree crease, vertex splitting), feature edges (35 degree dihedral plus boundaries), the LOD chain.

## Coordinate contract

x is width and y is height, both centred; z is depth from 0 at the mounting back face. Axes are matched to the catalog width/height/depth (permutation with a parity fix); without a catalog size the smallest extent is depth, the largest height. The mounting side is guessed from the larger flat face. When the catalog size and the model differ by more than 2 mm the model is scaled to fit the footprint.

## Budgets

`lods[0]` is at most 500 / 1200 / 2000 triangles (small, medium, large part), `lods[1]` at most 300, the last level is the 12 triangle bounding box. `Quality::Draft` is a much smaller first level for quick previews. `pick_lods` drops levels of the costliest parts until the scene fits a budget (design: 1.5 M triangles).

## Instancing

`group_instances` groups placements by part key (`geometry_key`, or `box_key` for fallbacks) into `InstanceBatch` so the renderer draws one mesh with many transforms.

## Cache

Per user, `dirs::cache_dir()/kopercad/part-geometry` (`%LOCALAPPDATA%` on Windows, no admin). One binary file per `{content_id}-a{ALGO}-{quality}{dimshash}`, versioned and length-checked; damaged or old files are rebuilt. Failures are cached (so a bad file is not retried on every start); cancellation is not. `prune` removes by age and size. `content_id` should be the sha256 of the imported source record.

## Service

`GeometryService` builds on a background thread with a 10 s limit per part and a wake callback for the UI. A failed key is not retried until `forget(key)`.

## Known limits

- B-spline surfaces are flat approximated (counted in `approximated_faces`).
- Spindle tori are broken.
- Wall diagonals of simple cylinders can survive as faint feature edges after decimation (tolerated in the test).
- Assemblies cover `MAPPED_ITEM` and the relationship form only.
- No per-part rotation override yet and no generated shapes (DIN rail, duct) yet.

## Try it

```
cargo run -p novo-interop --release --example step_mesh -- part.step [--obj out.obj]
```

prints per level triangle counts, feature edges, GPU size and mesh health (open and non-manifold edges, winding conflicts).

## Home

The module is in `novo-interop` because the crate list is fixed. The final home is the `import` feature of `novo-view3d`, or a geometry crate the coordinator adds; the API is pure functions plus an optional disk cache, so moving it is mechanical.

## Where the user meets it

Proposed entries (to be built by the UI integration threads, not in this PR):

- Parts library / part sheet: action "Import 3D model (STEP)" on a part, plus a status line "3D model: STEP, 480 triangles" or "Box from catalog size (reason)" using `MeshOrigin` and `Fallback`.
- 3D cabinet view: parts show their mesh while `GeometryService` builds (box first, mesh when `Ready`).
- Status of a failed model uses the cached failure message; "Retry" calls `forget(key)`.

Status of the engine: complete and tested; no UI yet. Whether maker meshes are wanted at all is the owner's decision (3d-view.md section 12); this PR is a draft until then.
