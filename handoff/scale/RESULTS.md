# Scale results (2026-10-21)

Measured with `scripts/scale-bench.sh` (release build, one cloud container core set, no GPU). Project: 300 generated motor starters = **600 pages, 2,130 devices, 8,100 wires, 6,000 symbols, 21,015 records**, 30 cabinets with plates, rails and every device placed. File size 6.6 MB. Generator: `crates/novo-tests/src/big.rs` (`BigSpec::SMALL/MEDIUM/LARGE`, or `--feeders N`).

Numbers are one run each, not averages. The generator is plain: repeated identical circuits, a few dozen distinct parts. A real project has more variety, so treat this as a lower bound on work per page, not a promise.

## What is fast (after the fixes below)

| Step | Time | Notes |
| --- | ---: | --- |
| Open file | 97 ms | SQLite read plus model build |
| Save whole project | 134 ms | |
| Autosave after one edit | 11 ms | |
| One edit / undo | under 1 ms | |
| Draw one page (scene build + tessellation) | 0.5 ms | all 600 pages: 43 ms |
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
| Heat, 2D steady (central run) on a 60-device cabinet | 9 s (was 25 s) | about 380 ms per solver iteration, 24 iterations. With uncertainty bands (3 runs) 16 s (was 54 s). |
| Heat, coarse 3D CFD on the same cabinet | 181 s | 125 x 126 x 21 cells. Not changed here. |
| `Project::netlist()` | 17 ms per call at 8,100 wires | Not cached; every caller pays it. Fine once, costly in a loop (see fix 1). |
| Design checks | 0.9 s | OK for a button; would be too slow to run on every keystroke at this size. |

The cabinet in the heat test is deliberately dense (60 placed devices on a 1.6 x 1.9 m plate). A cabinet with 15 devices will be much cheaper; I did not measure that.

## Fixes in this change

1. **Cable diagram and cable schedule**: 6.3 s each on 300 cables, because `cable_screen` rebuilt the netlist for every cable. `cable_screens` now builds it once. Both reports about 100 ms. All reports together went from 13.2 s to under 1 s. Test: `tests/scale_budget.rs`.
2. **2D heat solve stopped late**: the temperature change stalled near 0.02 K while the tolerance is 0.01 K, so every run used all 60 iterations and reported "not converged". A run whose change is within 10x the tolerance and has not fallen by a fifth over 8 iterations now counts as settled (`converged = true`, since 0.02 K is far inside the model's own error). The pressure solve tolerance went from 1e-9 to 1e-7. Thermal and cables test suites pass. Effect: 60 to 24 iterations, 2.7x faster.

## Not done (ranked)

1. 3D CFD coarse at 181 s needs the flow solver owners (it should run as a background job with progress; check what the UI does with a 3-minute wait).
2. 2D heat per-iteration cost (380 ms for 17,000 unknowns is slow; the pressure matrix is rebuilt from `Vec<Vec>` 15 times per iteration and solved with Jacobi CG).
3. A cached `Project::netlist()` per revision (touches `novo-core`, WS-02).
4. Design checks 0.9 s: incremental or per page.
5. The app itself: the benchmark covers engines and scene building, not egui frame time with a GPU. The real "pan and zoom at 600 pages" figure needs the QA harness on a real GPU.

## Budgets

`scale_bench --budget` fails when open, save, autosave, page scene, search, one edit or undo exceed the limits in `BUDGETS`. `tests/scale_budget.rs` runs in normal CI at 60 pages with generous limits. A release-mode job in CI is not added (would cost Actions minutes); run the script before releases.

## Where the user finds it

Developer tooling only: `scripts/scale-bench.sh` and `cargo run --release -p novo-tests --example scale_bench`. Fixes 1 and 2 show in the app as Reports mode (Cable diagram, Cable schedule) opening quickly on big projects and Heat finishing sooner.
