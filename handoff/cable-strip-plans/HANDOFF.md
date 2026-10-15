# Cable and terminal strip plans: handoff

Updated 2026-10-15. Crate: novo-reports (`schedule.rs`, `plugs.rs`). Branch `claude/project-thread-8920qu`.
- `cableschedule`: not proposed as a default generated page (`Report::proposed_by_default`), it is an export first.
- `plugs`: sections per `DeviceKind::Connector` device; pins from the device's symbols; destinations followed through junctions only.
- Tests: `crates/novo-reports/tests/plugs_schedule.rs`.
