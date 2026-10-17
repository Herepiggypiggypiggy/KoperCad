# Toolbar labels handoff

Branch `claude/project-thread-046o6k`. Code: `novo-app/src/app/dock.rs` (`dock_button`, `LABELS_MIN_CANVAS`), `novo-theme/src/widgets.rs` (`tool_button_labeled`), `settings.rs` (`DockLabels`), `preferences.rs`. Tests: `app/dock_labels_tests.rs`.
Behaviour: Auto shows words when the canvas is at least 720 wide (labelled dock is about 630 wide). Tooltips always carry name and key.
Resume: `cargo test -p novo-app dock`.
Not verified: Windows rendering, DPI scaling, a screenshot of the labelled dock on a real GPU.
