# Two wiring modes: roadmap
Scope: corner mode (EPLAN) and line mode (AutoCAD) for wires, a toggle with shortcut, and an engine rule that wires never overlap, in both modes. Design: docs/design/wiring-modes.md.
Defaults: rule lives in novo-editor/novo-checks (core API untouched); mode is a per-user setting in novo-app Settings.
1. Design PR (docs) - in progress
2. Engine PR 1: geometry::overlaps + nudge in draw_wire/transform
3. Engine PR 2: WiringMode, line behaviour, draw_wire_exact
4. novo-checks: wire.overlap
5. UI: setting, segmented control, Shift+W, wire.mode, DESIGN.md row (after Workflow/Layout answer)
Open: toggle placement and shortcut (proposed: Shift+W, dock control) awaiting Workflow cse_01JHYmmTDsVHDL7pEab8MGwy and Layout cse_01UXbCeQW22y6tv74F2Xvbvm.
