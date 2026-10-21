# Responsiveness: handoff (2026-10-21)

Branch claude/project-thread-p4a5ju, one PR "Responsiveness" (head SHA in PROGRESS.md). Not READY until the reviewer says so; after that do not push, open a new PR from main.

## What changed
- novo-core: `Project::netlist()` returns `Arc<Netlist>` built once per revision; new `Project::derived::<T>(build)` caches any derived result per revision (one slot per type). Public API change in novo-core: WS-02 / R2 should look at it. `Project` stays Clone; a clone shares valid entries.
- novo-app: terminal proposals via `derived` (the cause of ~100 ms per frame at 600 pages: whole-project `suggest_terminals` ran about ten times a frame); Operate list + "buttons on page X" cached in `LiveView.operate`; design checks on a worker thread over 1,500 symbols+wires (`checks.rs`: `refresh_problems`, `CheckJob`); pill says "Checking..." / "updating...".
- novo-workflow: `options::evaluate` walks tables by reference, returns early when nothing is off (#1211 hot path).
- QA: control `frame_stats`, qa step `frame_stats`, report section Measurements, `qa/scenarios/40-scale-frames.qa` (slow), `scale_bench --save-project`, `scale_bench` options steps. qa.sh: fixed the libxkbcommon check (grep -q + pipefail gave a false "missing").

## Numbers (600-page varied project, release, no GPU, software Vulkan)
Before / after, 1366x768, UI pass mean: 110 ms to 8 ms standing still; pan 102 to 9; zoom 106 to 9. Whole frame (includes software rasterising): 135 to 30 ms. At 1920x1080: UI pass 134 to 30, whole frame 171 to 70. Reports: /mnt/project-files/qa/responsiveness-scale600 (before) and responsiveness-scale600-after. Options per edit: evaluate 33 to 17 ms, machine copy 73 to 53 ms.
These are NOT real-GPU numbers: the whole-frame figure is the CPU rasteriser. Run the scenario on a Windows PC with a GPU for the real figure.

## Not done
- Machine option copy is still rebuilt per edit when documents are read (53 ms at 600 pages with ~1,350 objects left out): incremental update would need a change feed from the model.
- Design check is not cancellable or incremental: an older run finishes and is dropped. Per-page incremental checks remain open.
- Other whole-project work per frame may hide behind the next sample; re-profile with the recipe in RESULTS.md after big UI changes.
- Heat / CFD: already background with cancel; no change made.
- R1 review not needed (no solver touched).

## Resume
`cargo test -p novo-core -p novo-app -p novo-workflow`; scale file: `scripts/scale-bench.sh --size large --only generate --save-project target/scale600.novo`; frames: `KOPERCAD_BIN_DIR=target/release scripts/qa.sh --slow --only scale-frames`.
