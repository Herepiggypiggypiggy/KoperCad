# Handoff: device shapes
Code: `novo-view3d/src/device_shape.rs`. The key of a generated shape encodes family and size (half mm), so `device_shape::ensure(library, scene)` builds what a scene needs; the builder (`build/mod.rs`) assigns `Shape::Part(key)` for devices with a class; `Layers::shapes` strips them back to boxes. A maker model must use a normal `GeometryKey`; the tag byte 0xD5 marks generated ones.
App: `view3d.rs` calls `ensure` after each scene build; toggle in `view3d_controls.rs` (Layers menu).
Honest limits: details are typical, not the maker's; only the outer size is the part's.
Resume: `cargo test -p novo-view3d --all-features`.
