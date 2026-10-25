# EMC fix-it: handoff

Branch `claude/project-thread-la9yqo` (kopercad). PR: see PROGRESS.md.
Files: `crates/novo-emc/src/{report,rules,project}.rs` (Fix, preview, apply), `crates/novo-app/src/app/emc.rs` (window, preview card, `emc_preview`), `macro_actions.rs` (`macro_target_page`).
Tests: `cargo test -p novo-emc -p novo-app --lib -- emc macro`. QA: `scripts/qa.sh --only 22` (scenarios 220 bonding preview and apply, 221 light theme and Cancel, 222 pigtail, 223/224 macro from the contents page). Evidence: /mnt/project-files/emc/evidence-fixit/.
Traps: a finding is identified by rule plus subjects, so a fix only "clears" it when that pair disappears (EMC-11 stays while a device is also stated not bonded). Preview/apply "requirement" fixes write properties (`emc.finish`, `emc.bonded`, `emc.shield_a/b`): they state what the build must be, they do not change geometry. qa.sh needs `apt-get install openbox mesa-vulkan-drivers libvulkan1 libxkbcommon-x11-0`; Pages rows are painted, click by coordinates.
Unrelated: `22-learn-challenge-solved` fails at 1366x768 on main too (`Solved` not on screen).
