# SCALE-2 results: time budgets at 600 pages

Machine: cloud container, 4 vCPU Xeon 2.1 GHz (shared), 15 GB RAM, no GPU. Release build. One run each, not averages. Project: `BigSpec::new(300)`, varied: **600 pages, 2,676 devices, 8,513 wires, 6,418 symbols**, 7 MB file. Synthetic, no customer data.

## Where the numbers come from

* Engine steps: `scripts/scale-bench.sh --size large --budget` (see SCALE-1 in `handoff/scale/RESULTS.md`). All 30 steps now have a limit; none is over.
* The window: `cargo test --release -p novo-app --lib scale_tests -- --nocapture` (headless egui frames, CPU only, 1366x768). Frame time here is egui layout and our code, **not** GPU time. The GPU side is covered by QA scenario 40 on a real GPU.
* CI (debug build) runs the same test on 150 pages with the limits scaled (x12 for debug, x pages/600), so a cost that grows with pages x pages goes over while a slow CI machine does not.

## What the app did at 600 pages (before, after)

| Step | Before | After | Cause |
| --- | ---: | ---: | --- |
| Commissioning: one frame | 8,600 ms | 16 ms | The checks list rebuilt every target of the project for each of the 600 pages, on every frame |
| Commissioning: switch to it | 3,100 ms | 90 ms | same |
| Bench packet PDF | 19,000 ms | 1,450 ms | `build_sheets` cloned the whole report for each section (quadratic in sections) |
| Open file in the app | 323 ms | unchanged | engine open is 91 ms; the rest is the window setup |
| First frame after open | 216 ms | unchanged | |
| Schematic frame (CPU) | 16 ms | unchanged | |
| Go to a page and draw | 4 ms | unchanged | |
| Ctrl+K, tag, jump | 164 ms | unchanged | one search plus a page change |
| Switch to Cabinet / 3D | 12 / 18 ms | unchanged | |
| Switch to Reports | 456 ms | unchanged | refreshes all 12 live reports once (their engine times add up to about 600 ms) |
| Open Autopilot | 318 ms | unchanged | |
| Standards report PDF | 1,350 ms | unchanged | |

(An early "open file 5 s" was the test's own fault: it opened the file while the app still held its lock; fixed in the test.)

## Budgets (release, 600 pages)

Window: open 1.5 s, first frame 1 s, any steady frame 100 ms, page change 100 ms, Ctrl+K jump 500 ms, enter Cabinet/3D 500 ms, enter Reports 1.5 s, enter Commissioning 600 ms, Autopilot open 1.5 s, bench packet 5 s, standards report 5 s.
Engine: see `BUDGETS` in `crates/novo-tests/examples/scale_bench.rs`.

## Still slow, not fixed (ranked)

1. Bench packet 1.4 s and standards report 1.35 s: both call `wire_cut_jobs` (which routes every wire) three to five times; one shared result per revision would halve them. Needs a cache keyed by project revision (novo-core owner for the netlist idea).
2. Reports mode entry 0.45 s: runs on the UI thread; a background refresh would remove the pause.
3. `Project::pages_in_order` sorts all pages on each call; it shows up in samples of `occurrences` and the search. A cached order per revision belongs to novo-core.
4. Heat and CFD at large cabinets: unchanged from SCALE-1.
