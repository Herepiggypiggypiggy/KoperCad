# Handoff: maker parts XML importer

Branch `claude/project-thread-v1jdh3` (kopercad). PR: see PROGRESS.md.
Code: `crates/novo-parts/src/dataportal.rs`, tests `crates/novo-parts/tests/dataportal.rs`,
bulk routing in `bulk.rs` (`read_portal_files`, `upsert_merged`), dialog in
`crates/novo-app/src/app/eplan_parts.rs`, EPLAN-export route in `novo-interop/src/eplan/parts.rs`.
Resume: `cargo test -p novo-parts --test dataportal`, `cargo test -p novo-app eplan_ui`.
Traps: no real file was available; attribute matching is by name tokens (`terminal_field`,
`part_field`). Positions stay in the maker's frame on purpose. Source/format knowledge is in the
private mining notes; nothing from them is copied into the repo.
