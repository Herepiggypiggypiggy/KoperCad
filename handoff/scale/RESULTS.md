# Scale results

Two measured projects, both 600 pages, one run each (not averages), release build, one cloud container, no GPU. Run with `scripts/scale-bench.sh --size large` (varied, default) or `--uniform`.

* **Varied (update 2026-10-21, second PR)**: 300 circuits of five kinds: motor starters (0.37 to 11 kW, parts and cable size by power, cable length 5 to 80 m), three-phase heater groups, and I/O loops (4 to 12 sensors on an input module and valves on an output module). About 45 distinct parts, cabinets of different size and slot count (rails per plate height). **600 pages, 2,676 devices, 8,513 wires, 6,418 symbols, 23,385 records**, 29 cabinets, 7.0 MB file. Seeded, so the same every run. Full table: `large-run-varied.md`.
* **Uniform (first PR)**: the same motor starter 300 times (2,130 devices, 8,100 wires). A best case for caches and a baseline. Full table: `large-run.md`.

Still synthetic. Still no customer data, no imported EPLAN-style tags, no multi-level structure. Real projects will differ; treat these as order of magnitude, not a promise.

## Varied project against uniform (what changed)

The varied project has 5 percent more wires and 25 percent more devices, and some steps cost more than that: wire cut jobs 67 to 116 ms, wires report 70 to 110 ms, search 2 to 5 ms, design checks 0.86 to 1.0 s (6,204 problems against 2,762, since I/O and heater circuits have more open ends), Autopilot 217 to 226 ms (14,931 proposals), PDF 188 to 144 ms. No step went into seconds with the variety (largest: design checks, 1.0 s). The two hot spots found in the first PR were not specific to identical circuits.

## What "redraw" measures

The earlier line "redraw 0.5 ms" is the **model-side cost of one page only**: building the page scene from the model and tessellating it to triangles on the CPU. It is **not a frame**: no GPU, no egui layout, no upload, no rasterisation, and unchanged items come from the mesh cache. The benchmark now also does every one of the 600 pages with the real symbol artwork: 90 ms for all 600 (0.15 ms mean), slowest page 0.88 ms. A real frame time at 600 pages needs the QA harness on a real GPU (still open, item 5 below).

## What is fast (uniform project, after the fixes below)

| Step | Time | Notes |
| --- | ---: | --- |
| Open file | 97 ms | SQLite read plus model build |
| Save whole project | 134 ms | |
| Autosave after one edit | 11 ms | |
| One edit / undo | under 1 ms | |
| Page scene build + tessellation (model side only, not a frame) | 0.5 ms | all 600 pages: 43 ms |
| Search for a tag | 2.3 ms | |
| Number all 8,100 wires (plan / apply) | 50 / 28 ms | |
| Design checks | 857 ms | 2,762 problems |
| Wire cut jobs | 67 ms | |
| All 12 reports together | about 750 ms | largest single one 111 ms |
| Autopilot plan | 217 ms | 12,600 proposals |
| PDF export of all 600 pages | 188 ms | 7.1 MB. Uses library artwork. |
| Route all wires (whole project) | 14 ms | |
| 3D scene of one cabinet with wires | 29 ms | all 30 cabinets: 788 ms |

## What is slow

| Step | Time | Why |
| --- | ---: | --- |
| Heat, 2D steady (central run) on a 60-device cabinet | 9 s (was 25 s), stalled not converged | about 380 ms per solver iteration, 24 iterations. With uncertainty bands (3 runs) 16 s (was 54 s). |
| Heat, coarse 3D CFD on the same cabinet | 181 s | 125 x 126 x 21 cells. Not changed here. |
| `Project::netlist()` | 17 ms per call at 8,100 wires | Not cached; every caller pays it. Fine once, costly in a loop (see fix 1). |
| Design checks | 0.9 s | OK for a button; would be too slow to run on every keystroke at this size. |

The cabinet in the heat test is deliberately dense (60 placed devices on a 1.6 x 1.9 m plate). A cabinet with 15 devices will be much cheaper; I did not measure that.

## Fixes in this change

1. **Cable diagram and cable schedule**: 6.3 s each on 300 cables, because `cable_screen` rebuilt the netlist for every cable. `cable_screens` now builds it once. Both reports about 100 ms. All reports together went from 13.2 s to under 1 s. Test: `tests/scale_budget.rs`.
2. **2D heat solve stopped late**: the temperature change per iteration stalled near 0.02 K while the tolerance is 0.01 K, so every run used all 60 iterations and ended "not converged". A run whose change has stalled (within 3x the tolerance, not falling by a fifth over 8 iterations) now stops early, **but is not marked converged**: it sets a new `stalled` flag on the result, and the report adds the note "The solver stopped when the change stalled just above its tolerance; the numbers may still move by up to about 1 K". Change per iteration is not error: on the 20-feeder cabinet (60 devices) the old 60-iteration answer and the new 24-iteration answer differed by up to 0.63 K in device surface and 0.73 K in local air (R1's measurement). The old 60-iteration run was not converged either, so neither is a reference. The pressure solve tolerance went from 1e-9 to 1e-7. Effect: 60 to 24 iterations, 2.7x faster. Tests: stall rule and note in `thermal.rs` / `report.rs`.

## Not done (ranked)

1. 3D CFD coarse at 181 s needs the flow solver owners (it should run as a background job with progress; check what the UI does with a 3-minute wait).
2. 2D heat per-iteration cost (380 ms for 17,000 unknowns is slow; the pressure matrix is rebuilt from `Vec<Vec>` 15 times per iteration and solved with Jacobi CG).
3. A cached `Project::netlist()` per revision (touches `novo-core`, WS-02).
4. Design checks 0.9 s: incremental or per page.
5. The app itself: the benchmark covers engines and scene building, not egui frame time with a GPU. The real "pan and zoom at 600 pages" figure needs the QA harness on a real GPU.

## Budgets

`scale_bench --budget` fails when a step exceeds its entry in `BUDGETS`: open file 3 s, full save 5 s, autosave 0.5 s, page scene (one page) 100 ms, page scene + tessellate (one page) 100 ms, page scene + tessellate (every page, 600 pages) 1 s, search 100 ms, one edit 100 ms, undo 100 ms. Each row has its own entry; the first matching prefix wins. `tests/scale_budget.rs` runs in normal CI at 60 pages with generous limits. A release-mode job in CI is not added (would cost Actions minutes); run the script before releases.

## Where the user finds it

Developer tooling only: `scripts/scale-bench.sh` and `cargo run --release -p novo-tests --example scale_bench`. Fixes 1 and 2 show in the app as Reports mode (Cable diagram, Cable schedule) opening quickly on big projects and Heat finishing sooner.

## 2D heat solver: temperatures before and after (kelvin)

The first PR made the 2D solver stop sooner (stall rule, 3x tolerance) and loosened the pressure solve (1e-9 to 1e-7). Before = main before #1173 (`158dfc1^`), after = main now. Same example (`crates/novo-thermal/examples/validation_temps.rs` and `crates/novo-tests/examples/heat_dense_temps.rs`), central run, coarse grid as in the tests.

| Case | Mean air rise before / after (K) | Air at top (C) | Hottest surface (C) | Mean device air (C) | Iterations before / after |
| --- | --- | --- | --- | --- | --- |
| closed box, 120 W spread | 8.382 / 8.382 | 34.018 / 34.018 | 72.001 / 72.001 | 34.727 / 34.727 | 22 / 22 |
| closed box, 40 W hot device | 2.883 / 2.883 | 28.056 / 28.056 | 112.606 / 112.606 | 30.161 / 30.161 | 16 / 16 |
| two vents, 120 W | 5.795 / 5.795 | 32.500 / 32.500 | 70.294 / 70.294 | 32.156 / 32.156 | 16 / 16 |
| fan pair, 200 W | 3.696 / 3.696 | 30.372 / 30.372 | 89.569 / 89.569 | 29.266 / 29.266 | 10 / 10 |
| stacked pair, 2 x 30 W | 4.278 / 4.278 | 29.512 / 29.512 | 95.530 / 95.529 | 31.284 / 31.284 | 15 / 15 |
| side by side pair | 4.389 / 4.389 | 29.034 / 29.033 | 95.646 / 95.646 | 31.018 / 31.018 | 12 / 12 |
| fan box, one fan | 1.925 / 1.925 | 27.128 / 27.128 | 117.598 / 117.598 | 27.907 / 27.907 | 11 / 11 |
| fan box, two vents | 6.949 / 6.949 | 33.351 / 33.351 | 123.471 / 123.471 | 35.362 / 35.362 | 16 / 16 |
| dense cabinet, 60 devices, 100 W (low load, default grid) | 2.308 / 2.308 | 27.367 / 27.366 | 36.522 / 36.521 | 28.050 / 28.050 | 19 / 22 |
| generated cabinet, varied, 69 devices, 132 W | 4.895 / 4.887 | 40.052 / 40.048 | 74.649 / 74.659 | 41.922 / 41.872 | 60 (not converged) / 23 (stalled) |
| generated cabinet, uniform, 70 devices, 117 W | 2.800 / 2.797 | 38.945 / 38.940 | 55.430 / 55.442 | 38.266 / 38.264 | 60 (not converged) / 30 (stalled) |

Reading it: the nine cases that converge normally (the eight physics cases and the dense low-load cabinet) move by 0.001 K or less (only the looser pressure tolerance acts on them). In these two generated-cabinet cases, which used to run all 60 iterations unconverged, the answer moves by at most 0.05 K (mean device air, varied cabinet) and the run now stops at 23 and 30 iterations. That 0.05 K holds for these two cabinets only; a stalled run can move by several tenths of a kelvin. On the 20-feeder cabinet (60 devices) of the first PR, the old 60-iteration answer and the new 24-iteration answer differed by up to 0.63 K in device surface and 0.73 K in local air (R1's measurement); neither is a reference, which is why stalled runs are marked stalled and not converged. The dense low-load cabinet is the one case that needed more iterations after (22) than before (19): the looser pressure tolerance changes the count either way, and it did not stop sooner there. Wall time of the varied cabinet, central run: 14.6 s before, 6.0 s after; uniform 28.8 s, 13.2 s.
