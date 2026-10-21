# Clearance zone in 3D

- Selected device with stated clearances gets a dashed outline of its footprint grown by the need on each side (`dimension::clearance_zones`, painted in `widget.rs`, shown with the Dimensions toggle). #1134.
- Follow-up (reviewer R3's nits): the Dimensions tooltip names the dashed rectangle; `needs` is already in plate sides (`Placement::need` turns the part's data-sheet sides with R), pinned by a build test and a dimension test with a turned device; stroke is 1.5 px, alpha 230.
- Seen in the real window at 1366x768 in the dark and the light look (headless QA run, K1 of the example selected, front view): readable on both. Not seen on Windows.
- Known limit: the zone shows the selected device's own need only. The gap lines use the larger of its need and the neighbour's facing need, so a neighbour that asks for more can be too close without crossing a dashed line.
- Next: tour button, pinned labels with Devices off, duct fill % in hover.
