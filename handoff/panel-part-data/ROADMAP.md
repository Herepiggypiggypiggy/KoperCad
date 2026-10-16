# Panel part data (R0 + R1)

Scope: `docs/PRO-PANEL-GAP-ANALYSIS.md` R0 (part keys) and R1 (clearances in the check).

Done (draft PR from `claude/project-thread-qxuwc2`):
- `novo-parts`: keys `power_loss_w`, `mounting`, `clearance_{above,below,left,right}_mm`; `Mounting`, `PanelData`; catalog format 2 migrates format 1; CSV import maps legacy/German headers; property sheet lists the keys on every assigned part and validates input.
- `novo-layout`: `Margins`/`Side`, per-side `Placement::need`, `clearance_violations` uses facing sides; `model::device_margins` reads device override then part.

Next: BMEcat/ETIM reader fills the keys (#356); R3 heat estimate reads `PanelData`; R7 planner reads margins; part browser column.
