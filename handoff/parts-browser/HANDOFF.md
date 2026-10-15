# Handoff: Parts browser

Branch `claude/project-thread-x7q4jr`, PR #429 (draft). Code: `crates/novo-app/src/app/parts.rs` (window, detail), `parts_import.rs` (hand-off import); command `parts.browser` (palette: "parts").
Done: browser, maker filter, detail blocks (keys grouped by words, so #419's power_loss/clearance keys show), Import button opens the maker page and watches Downloads (1 s poll, folder changeable), drop a file or Choose file, status strip with progress/result/failure, STEP size and BMEcat parts saved to the user catalog, source record shown with Remove.
Not done: EDZ XML and DXF members are stored but not read (needs novo-interop; talk to the DXF/EPLAN and 3D geometry threads); no fetch adapters (`NoFetcher`), so every maker is hand-off; watch folder is not persisted; first-use notice UI is not needed until a maker is fetch-enabled.
Resume: `cargo test -p novo-app parts`.
