# Duct fill in 3D: handoff
Where: 3D tab > Layers > Duct fill. Tones come from `cabinet_routes::tone` (idle keeps its colour, ok green, above 80% amber, over red, no size grey); `view3d_duct_fill::tint` sets `Item.tint` on Duct items matched by rail id. Fills come from `cabinet_routes::duct_fills`, cached per structure and revision in `view3d.rs`.
Not verified: on-screen look in a real window; the match of Duct item id to rail id rests on the scene builder using the layout duct id.
