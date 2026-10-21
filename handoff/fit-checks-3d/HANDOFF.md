# Fit checks in 3D: handoff
Where: 3D tab > Dimensions, select one device. A gap line between two devices reads e.g. `50 mm (min 60 mm)`; red when smaller than the larger of the two facing needs, green otherwise.
How: `Item::needs` ([left, right, above, below] mm, from `Placement::need`) set by the builder; `dimension::dimensions` sets `Dimension::required`; the widget colours by `clearance_ok()` (MARK_ERROR / MARK_OK, synced with the theme tokens by a test).
Same rule as `novo_layout::clearance_violations` for side-by-side pairs; the diagonal case and plate edges are not judged here.
Not seen on screen: a red or green gap (the example project's devices are far apart); only the unit test covers the colours.
