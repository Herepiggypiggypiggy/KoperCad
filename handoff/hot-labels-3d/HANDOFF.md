# Pinned hot-spot labels in 3D: handoff
Where: 3D tab > Heat (simulate), tick Devices. `heat_3d::pin_labels` (called by `tint_devices`) rewrites the label of the top `PINNED` (3) devices from `heat_spots::rows` (over limit first, then hottest) to `-F1  54 °C`. The source scene keeps its plain tags. The list is the same one as the Hot spots menu.
