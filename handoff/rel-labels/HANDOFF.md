# REL-LABELS handoff

Branch `claude/project-thread-gtjd91`, one PR per slice from main. Slice 1 PR: see PROGRESS.md.

State: slices 1 and 2 merged (#969, #970); slice 3 (Calculations badge, Dutch basis words) in review.

Resume: read `release/v0.1-plan.md` section 8 (model versus exact), then ROADMAP.md "Next". Add a new number to `Model` and `ROWS` in `novo-app/src/app/model_labels.rs` first; the page and the guard test pick it up.

Traps:
- Messages are asserted verbatim in `novo-checks/tests/{checks,motor_starter}.rs`; update them with the text.
- Cloud disk fills up: use `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0` and `rm -rf target` between runs; a linker "Bus error" means disk full.
- Heat balance note lives in `novo-cfd/src/cabinet/mod.rs` (`FAN_BALANCE_NOTE`). ENCLOSURE_VALIDATED stays false.
- UI colours: `crate::theme::ui_palette()`, never `tokens::*`.

Contacts: reviewer R2 (data/engines) cse_01SEokxUZ1MKDwyQvLm4DPys; coordinator session_01Na1FZm8gtMxPVXh4qycirm.
