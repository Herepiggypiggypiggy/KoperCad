# Roadmap: 3D and Cabinet panels, layers and toggles

Scope: one state per 3D option, one way to open and close every panel, no orphan panels, always-visible simulation status. Design: `docs/design/visual-redesign-3d.md` (#1583), audit `docs/design/3d-panels-audit.md`.

Done: audit; PR A (ViewOption registry, EMC off, one heat switch, commands through the registry, menu rows show the value).
Next, in order: status chip (shared by Cabinet and 3D, on the heat thread's `SimStatus`); PR B side panel with foldable, undockable sections (Inspector, Layers, Heat, Vents and fans, EMC, Door, Sizing, Wire runs) with Sections menu, Esc and Ctrl K "Show section"; PR C 3D rail and flyouts when lane V3a lands; Cabinet EMC unification.
Open: Temperature vs Heat (designer says same data, confirm in code); window rules from lane V1/#1576.
