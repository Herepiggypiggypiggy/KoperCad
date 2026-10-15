# Commissioning thread: handoff

- Branch `ws-commissioning-design`, first PR is the design doc only. No build requested by the owner.
- Decision: state lives in side tables in the project file (storage schema v3), not in novo-core
  records, so WS-02's API is untouched and ticks are not undoable design edits.
- Decision: the report is a `novo_reports::ReportOutput`, so PDF and CSV reuse existing export paths.
- Needs: new crate `novo-commissioning` registered by the coordinator (CLAUDE.md rule).
- Resume: read `docs/design/commissioning.md`, then follow the build order in it.
