# T3: 3D view flags: handoff

Branch `claude/project-thread-75u6b0`. One PR from main: "Problem triangles T3: 3D view flags". QA scenarios 9430 (1366x768) and 9431 (1920x1080), fixture `qa/fixtures/cabinet-3d-no-room.novo` (regenerate: `cargo test -p novo-app --lib make_no_room_fixture -- --ignored`).

Shared painter API (for T2, T4, T6): `app/view3d_flags.rs`
- `Layer { view, rect, scene, extra }`, `paint(ui, &layer, &[&Flag]) -> Option<Hit>`: triangle per anchor, 14 pt, clusters within 14 px (count badge), faded 35 % behind a part, on top when selected, Tab and Enter work. `view3d.rs` calls it with the whole store, so a lane only has to put flags in the store.
- `world_of(scene, extra, anchor)`: Device, Duct, Rail, Door, Enclosure (front face), Plate, World; Opening and Wire come from `extra` (the view passes the vent markers as Opening).
- `Transients::refuse(title, lines, anchors)` for a refused edit (gone on the next press or key, 8 s at most, never counted); `typing(...)` for a problem that lasts while a value is typed.
- `glyph(painter, at, flag, pal)` and `novo_theme::widgets::flag_triangle` for a cue without the store.
Traps: schematic Wire ids are never matched to 3D wire runs. The old in-view item mark (black circle with a cross) still draws next to the triangle until T1. Census file `crates/novo-app/census/view3d.txt` only goes down.
