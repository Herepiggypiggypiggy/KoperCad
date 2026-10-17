# Layout duct check: roadmap

Scope: a duct lying over a rail or a device is a problem; Place all keeps clear of ducts already on the plate (QA finding, qa/cabinet-3d-heat.md second pass).

Done
- `novo_layout::duct_overlaps` and `Report::duct_overlaps` (rail centre line against duct footprint, device footprint against duct).
- Cabinet problems pill and device lines, Issues list (`ProblemKind::DuctOverlap`), bench report row.
- `place_all_around`: free regions around ducts, biggest first, leftovers go to the next region.

Next / open
- Place all ignores a horizontal duct only by region cut; zone order holds inside a region, not across a duct.
- Rails are checked as a centre line, not the 35 mm body.
