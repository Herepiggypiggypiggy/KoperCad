# Cable runs in 3D: handoff

Updated 2026-10-21 UTC. PR #1205 merged; panel and follow-ups in PR 2 (see ROADMAP.md).

## State
- Branch `claude/project-thread-168e6g` (repo herepiggypiggypiggy/kopercad), PR 1 with the Run card, lit path, duct outlines, entry marker, Not drawn list. Head SHA and review state are in PROGRESS.md.
- Code: `crates/novo-app/src/app/view3d_runs.rs` (basis, overlays, card), `view3d_wires.rs` (facts, list, hover), hook-up in `view3d.rs` (`ensure`, `run_marks`, card call). Tests: `view3d_runs.rs` unit tests, `view3d_runs_tests.rs` app tests. QA: `qa/scenarios/31-cable-runs-3d.qa`, pictures in /mnt/project-files/qa/cable-runs-3d/.
- No change to `novo-core`, `novo-routing` or `novo-reports`.

## Design in one paragraph
Geometry is the routing engine's (`route_project`, entry legs). The length and basis come from the wire list's own measurement (`novo_reports::wire_cut_jobs`), read lazily once per project revision (`view3d_runs::ensure`) because it routes again; so the card, cut list and wire list agree. The list and the card are one panel (`view3d_runs::panel`, `panel::Panel`, right side under the cube). Overlays (lit path 4 px, duct outlines 2 px, entry marker) are rebuilt when the revision, pose or selection changes.

## Resume
```
export CARGO_TARGET_DIR=/home/claude/target
cargo test -p novo-app view3d_runs
KOPERCAD_BIN_DIR=$CARGO_TARGET_DIR/debug scripts/qa.sh --only "Cable runs" --out /mnt/project-files/qa/cable-runs-3d/
```
QA on a fresh cloud box needs `apt-get update && apt-get install -y xvfb openbox mesa-vulkan-drivers libvulkan1 libxkbcommon-x11-0`. The `qa.sh` library check is flaky (ldconfig pipe with pipefail): rerun when it claims libxkbcommon-x11-0 is missing right after installing it.

## Traps
- The scene redraws only when `scene.revision` changes: overlays go through `view.set_overlays` (the `ghost_changed` path), not the scene.
- `check_layout` reports rows inside the Wire runs scroll area as overlapping the footer (heuristic false positive, S4), so scenario 31 does not call it.
- Once the PR is READY do not push to it; follow-ups go in a new PR from main.

## Honest labelling
Length figures are the wire list's: routed lengths are a model of the placed devices and ducts; a cable length is as stated; the outside part of an entry run is a stated figure and is never drawn. Bend radius and clearance are not checked (no data in the model) and the card says so.
