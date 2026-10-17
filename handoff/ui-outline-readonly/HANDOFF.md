# ui-outline-readonly handoff

Branch `ui-outline-readonly`, one PR from main. Engine untouched (no novo-core change).

Code
- `crates/novo-app/src/app/outline_items.rs`: pure wire and cable rows (tests inside).
- `crates/novo-app/src/app/outline.rs`: `net_sections` draws them under the tree; `NetJump` handles the click through `go_to_wire`.
- `crates/novo-app/src/backend.rs`: `open_read_only`, `is_read_only`, `BackendError::ReadOnly`; read-only keeps no session.
- `crates/novo-app/src/app.rs`: `Action::OpenReadOnly` (`file.open_read_only`), `enforce_read_only`, title and status text.
- Command registered in novo-workflow `commands.rs` (group Project, so File menu) with Dutch words in `language.rs`.
- Tests: `read_only_tests.rs`, `outline_tests.rs`, `backend.rs`.

Resume: `cargo test -p novo-app -p novo-workflow`.
