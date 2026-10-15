# PLC link: handoff

Branch `claude/project-thread-expes9` (kopercad), PR from it to main. Code in `crates/novo-plc/src/link/`, tests `crates/novo-plc/tests/link.rs`, app in `crates/novo-app/src/app/plc_link.rs` (+ gate1 test `plc_tags_leave_as_files...`), CLI `export plc`.

State: engine, UI and CLI done and tested. Formats other than CSV are untested against the real tools (see docs/design/plc-link.md table).

Resume: `cargo test -p novo-plc -p novo-cli -p novo-workflow`, then `cargo test -p novo-app plc_tags`.

Traps: addresses must never be written back by an import (plc.md §6). Names dedupe across the whole export, and the import compares against the exported name so `_2` suffixes and Studio 5000's 40-character cut are not read as renames. Lane B (safety) lives in a separate thread; keep it out of `link/`.
