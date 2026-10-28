# Handoff: plate follow

Branch `claude/cabinet-plate-follow`. Code: `crates/novo-layout/src/model.rs` (`rails_following`, `set_plate_following`), `Ask::SetPlateSize` and `Ask::ToggleTable` in `crates/novo-app/src/app/cabinet.rs`. A rail or duct within 2 mm of both old edges follows the new edge; a shorter one is only shortened when it would stick out. Nothing moves. Tests: `novo-layout/tests/model.rs`, `cabinet_tests`.
