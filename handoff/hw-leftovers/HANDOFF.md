# Handoff: hardware test leftovers

State (2026-10-28): three PRs open, all from main, none READY yet: #1624 (branch claude/project-thread-j76j2s), #1626 (claude/hw-leftover-cabinet), #1628 (claude/hw-leftover-s4). All three are even numbers, so merge lead A.

Notes with the reasons for every S4 item not fixed: project files `qa/ws-leftover-1/NOTES.md`. Evidence pictures: `qa/ws-leftover-1/pr-a/` and `pr-b/` (each with HEAD.txt).

Resume: `git fetch origin <branch>`; build with `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0` (debug info and incremental caches fill the cloud disk; `scripts/free-disk.sh`). Scenario runs: `scripts/qa.sh --only <name> --size 1366x768 --out <dir>`; the QA box needs openbox, mesa-vulkan-drivers, libxkbcommon-x11-0.

Traps: PR #1624 and #1628 both touch `crates/novo-cli/src/lib.rs` (different places); #1626 and #1616 may meet in `novo-thermal` heat start checks (BridgeError); a new BridgeError variant `TooLarge` is in #1626. Once a PR is READY do not push to it; follow-ups go in a new PR from main.


## State (2026-10-28 evening)
#1624 and #1628 are merged; #1626 is closed (superseded by #1625). The follow-up PR on `claude/hw-followups` carries the last three items. Still open and not mine: drawer resize (#1625). Not fixed on purpose: see `/mnt/project-files/qa/ws-leftover-1/NOTES.md`.
