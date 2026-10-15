2026-10-15 17:15 UTC Thread started by the coordinator: DXF import/export and EPLAN parts import as pure engines.
2026-10-15 17:30 UTC Wrote docs/design/interop.md.
2026-10-15 17:50 UTC novo-interop crate with dxf model, writer, reader; 23 tests pass; fmt and clippy clean.
2026-10-15 17:55 UTC PR #344 opened (DXF engine + design), reviewer messaged.
2026-10-15 18:20 UTC eplan module (XML + delimited import), 10 tests; PR 2 stacked on #344.
2026-10-15 17:30 UTC #344 READY on ac1c3aa (main reviewer). Its optional hardening (entity cap, INSERT array warning, tests/dxf_limits.rs) rides #347 since #344 may not be pushed to.
2026-10-15 17:40 UTC #347 merged by the lead. #354 (bridge: page to DXF) opened, 8 tests; merged main, handoff rewritten.
