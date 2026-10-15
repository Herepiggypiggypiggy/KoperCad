# Cable and terminal strip plans: handoff

Updated 2026-10-15. Crate: novo-reports (`schedule.rs`, `plugs.rs`). Branch `claude/project-thread-8920qu`.
- `cableschedule`: not proposed as a default generated page (`Report::proposed_by_default`), it is an export first.
- `plugs`: sections per `DeviceKind::Connector` device; pins from the device's symbols; destinations followed through junctions only.
- Tests: `crates/novo-reports/tests/plugs_schedule.rs`.

## How to reach it
- CLI: `kopercad-cli export cables <file> [-o out.csv|.xlsx]` (cable schedule), `kopercad-cli export plugs <file>`; `export reports` includes both.
- Generated page: `plugs` is proposed with the default generated pages when a project has connectors (live, in PDF). `cableschedule` is not a default page; a Generated page with generator `cableschedule` can be added by hand.
- Missing (UI integration thread): Reports/Export menu entries for both in the app.
