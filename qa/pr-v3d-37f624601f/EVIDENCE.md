# PR #1690 evidence: V3d Reports and Commissioning rails

- Code head: 37f624601f12a2aaa38acfa6a6733bdb234c54f2 (merge of main a4200d1e+ with the rail rows moved to rail_reports.rs). Clean tree, program built from it (unix 1793277282), report.md says "source at git 37f624601f".
- Scenario 7400-reports-commissioning-rails at 1366x768 and 1920x1080: 2 runs, 0 failed, 0 findings.
- Shots (viewed by the author): Reports rail, Export flyout open (Excel, report PDF, BOM, PDF), Commissioning rail on the Punch tab with the Punch row marked, back to Schematic.
- Tests at this head: fmt and clippy -D warnings clean for novo-app; `cargo test -p novo-app --lib -- rail pinned_navigator` 69 ok. Full workspace run is CI's.
- The PR head after this is the code head plus this folder only (qa/pr-v3d-37f624601f/: HEAD.txt, report.md, EVIDENCE.md); old qa/pr-v3d-03d4830654 removed from the repo.
- Screenshots, logs and results.json: this mount folder.
