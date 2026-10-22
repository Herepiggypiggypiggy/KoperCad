# Handoff: real-machine fixes RM-05 and RM-12

Branch claude/project-thread-8ed18z, one PR. Both fixes have unit tests and QA scenarios (61-plc-siemens-analog, 61-potential-chip-low, 61b-potential-chip-low-1920); evidence in /mnt/project-files/qa/rm05-rm12/.
- RM-05: read_address in crates/novo-plc/src/generate/list.rs strips the P of PIW/PQW/PEW/PAW.
- RM-12: egui moved a fixed-position menu to stay on screen but not its hit areas, and the wrapping chip row made the menu change height every frame. The menu now places itself from last frame's size and the chip rows do not wrap (crates/novo-editor/src/view.rs, context_menu).
- Trap: the cloud disk filled up; use CARGO_INCREMENTAL=0 and scripts/free-disk.sh. novo-cli tests were not run locally (CI runs them).
