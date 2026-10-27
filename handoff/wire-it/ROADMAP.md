# Wire-it roadmap
Scope: the guided wiring pass (audit/creative/round37.md idea 1). Design: docs/design/wire-it.md.

Done (PR "Wire-it", branch claude/project-thread-i16fyt):
- Engine `novo_reports::wireit`: bench sequence, duct fill after each wire, progress keys on the wires.
- Route per job (`WireCutJob::path`, `RunPath`) so the picture shows the route the cut length was measured on.
- 3D strip (`view3d_wire_it.rs`), lit path and both ends, camera framing, done wires in green.
- Landscape PDF list (`novo-pdf/src/wireit.rs`), command `export.wireit`.
- Commands `view.3d_wire_it`, `_next`, `_previous`, `_done`; menu View > 3D; Layers toggle.
- Tests in novo-reports, novo-pdf, novo-app; QA scenario 520.

Next (not built):
- Idea 2: continuity sheet in wiring order feeding Commissioning (depends on the order here).
- Half-page cards, a tablet mode, a key for "Run, next wire".
- Wires inside a cable: one step per cable ("pull the cable once") instead of one per core.
- A jump from a clicked wire in 3D to its step.
