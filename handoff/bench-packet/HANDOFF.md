# Bench packet: handoff

Branch `claude/project-thread-uj16jb` (kopercad), one PR from main.

Code: `crates/novo-reports/src/packet.rs`, `crates/novo-reports/tests/packet.rs`, `crates/novo-app/src/app/bench_packet.rs` (+ `_tests`), `report_outline` in `crates/novo-pdf/src/report.rs`, `cabinet_sections` in `app/assembly.rs`, workshop pack in `app/workshop.rs`, command `export.bench_packet` (commands.rs, menus.rs, language.rs, workbench.rs, menu_bar.rs), QA scenario `qa/scenarios/38-bench-packet.qa`.

Decisions
- The packet's wires appear once, on the circuit sheets; the assembly `:wire` tables are filtered out of the packet (the build order still lists wire steps).
- Cover says what is exact and what is an estimate (lengths with ~ come from `is_estimate`), and that 1:1 files print at 100 %.
- Packet order: cover, cabinet build, cut list, ferrules, circuit sheets, terminal plans.
- 1:1 files stay separate (they cannot share a document with A4 tables).

Resume: `cargo test -p novo-reports --test packet`, `cargo test -p novo-app --lib bench_packet`, `scripts/qa.sh --only bench-packet`.
Evidence: /mnt/project-files/qa/bench-packet/ (1366x768 and 1920x1080 shots, sample PDF).
