# Real part data import: handoff

Branch `claude/project-thread-pl7diy` in `Herepiggypiggypiggy/kopercad`. PR: see PROGRESS.md (head SHA is sent to the reviewer after each push).

## State
Feature complete for CSV and `.xlsx` tables: engine (`crates/novo-parts/src/table{file,map,preview}.rs`),
dialog (`crates/novo-app/src/app/parts_table.rs`), command `parts.import_table`, docs
`docs/PARTS-TABLE-IMPORT.md`, QA scenario 21. Tests: `cargo test -p novo-parts --test table_import`,
`cargo test -p novo-app parts_table`, `cargo test -p novo-workflow`.

## Resume
```
export CARGO_TARGET_DIR=/home/claude/target
cargo test -p novo-parts -p novo-workflow && cargo test -p novo-app parts_table
cargo build -p novo-app -p novo-cli && KOPERCAD_BIN_DIR=$CARGO_TARGET_DIR/debug scripts/qa.sh run --only parts-table --out /mnt/project-files/qa/parts-table-import
```
The QA box needs `apt-get update && apt-get install -y xvfb openbox mesa-vulkan-drivers libvulkan1 libxkbcommon-x11-0`.
`qa.sh` with `KOPERCAD_BIN_DIR` set does not rebuild: build first.

## Decisions and traps
- The shared `parse_delimited` delimiter detection now votes over 30 lines (it used the first line). Other CSV paths benefit; tests in csv.rs.
- Menu cap: the File > Import submenu may hold 12 items; parts imports live in File > Import parts.
- Nothing is guessed: bad values are dropped from the part and reported; unit assumed only when neither header nor cell has one, and the preview says so.
- "This file is" defaults to Not stated, so values carry no maker source unless the person says so.
- `recipes::{connection_kind, lifecycle_of, currency_of}` became `pub(crate)` for reuse.
- Real maker files have not been seen: suggestion words come from public wording and made-up samples.
