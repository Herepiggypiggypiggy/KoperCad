# Panel layout intelligence: handoff

Branch `claude/project-thread-o2407u` (restart from main after each merge). Engine code in `crates/novo-layout`; design in `docs/design/cabinet-layout.md` section 5b.1.

Resume: `cargo test -p novo-layout && cargo clippy --workspace --all-targets -- -D warnings`.

Decisions
- No `novo-core` change: duct class is the rail property `duct_class`; surfaces will be placement properties.
- The check judges relative zone order, not absolute bands, so a plate of only top-zone devices is not "wrong".
- Separation is given up before a device is left off (never silent: `Explanation::separation_relaxed`).
- Defaults (80 mm drive room, 40 mm duct separation, 25 W hot limit, 2000 mm stock, 2 mm kerf) are practice values, not standards.

Coordinate with: Enclosure depth and door (#409, `enclosure.rs`), Cabinet UI thread (cabinet.rs), Heat (#406).

## Lane B plan (not started; wait for #409 to merge, then restart the branch from main)
Build on `enclosure.rs` (#409: `Enclosure`, `Door`, `Hinge`, structure property keys, `CabinetLayout.enclosure/depths`). No `novo-core` change: store everything as free-form properties.
1. `surfaces.rs`: `Surface { kind: Plate|Door|LeftWall|RightWall|Roof|Floor|Profile, origin, u, v, normal, width, height, margin }` in the 3D frame of enclosure.rs (x across, y down, z from back wall to door). Each surface has its own `CabinetLayout`; a placement/rail names its surface with the property `surface` (absent = main plate); surface sizes are structure properties `surface.<name>` = "w h margin"; extra plates `plate2`, `plate3` for multi-plate cabinets.
2. `collide.rs`: pure triangle-mesh collision (SAT/Moller triangle-triangle with a BVH, plus triangle-triangle distance for clearance), meshes from part geometry (#449 STEP to render mesh, `novo-view3d::mesh::MeshData`; convert, layout must not depend on view3d). Box fallback from width/height/depth when a part has no mesh, flagged as such.
3. Door swing: sweep door-mounted devices through 0..max_angle in steps against plate devices and walls (uses `Enclosure::arc`/hinge); report the first angle at which they touch.
4. Wiring holes: `Opening { surface, rect }` keep-outs that devices must not cover, and the pass-through between plates for routing (`novo-routing`).
5. Rules: `Place all` per surface (door gets its own rule set section, e.g. `surface door`), violations name the rule.
6. UI: surface picker in the Cabinet header, 3D view shows surfaces (coordinate with the Cabinet UI and 3D threads).
Validation: curated reference cabinets with hand-checked layouts; property tests that any accepted placement has no mesh collision and the door closes and opens.
