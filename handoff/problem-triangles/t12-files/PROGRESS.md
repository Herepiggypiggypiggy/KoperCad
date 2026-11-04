# T12: progress

- 2026-11-04 PR 1: job flags. Failed exports and imports (STEP, AML, EPLAN, DXF, PDF, spreadsheet, workpack, bench, bench packet, handover, report, change report, variant, designer, manual, parts import, revision issue/lock/restore, sheet import) are red or yellow job flags in the top bar. Toasts 66 to 18 in the area. QA 9522/9523.
- 2026-11-04 PR 2: bench. `novo_theme::widgets::flag_chip` (triangle plus short title, sentence in a popover). Before-you-print gaps and the cut/ferrule/label tab banners are chips; ferrule/label export refusals are job flags (export.bench_cut, export.machine_csv, export.ferrules, export.labels); `bench_write` takes the job key. Census 18 to 13. QA 9524/9525.
