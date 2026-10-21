# End-user acceptance: handoff

State (2026-10-21 evening): round 2 done, fixes in one pull request from branch `claude/project-thread-ty2h8s`
(scenarios `58-acceptance-plc-card.qa`, `59-acceptance-multipage.qa`). Findings: `/mnt/project-files/audit/acceptance/round2-findings.md`,
pictures in `audit/acceptance/round2/`.

Resume
- Build: `export CARGO_TARGET_DIR=/home/claude/target; cargo build -p novo-app -p novo-cli`.
- Needs `apt-get update && apt-get install -y xvfb openbox mesa-vulkan-drivers libvulkan1 libxkbcommon-x11-0`.
- Run a scenario against the debug build: `$CARGO_TARGET_DIR/debug/kopercad-cli qa run FILE.qa --size 1366x768 --size 1920x1080 --out DIR --app $CARGO_TARGET_DIR/debug/kopercad`
  (`scripts/qa.sh` looks for `target/` in the repository, so call the CLI directly when the target dir is elsewhere).
- Exported files land under the run's profile `~/Documents/KoperCAD/` (`/tmp/kopercad-test-user-<pid>/home/...`) until the next run removes it: copy them out, then `pdftotext -layout`.

Traps
- `click_text Pages` fails at 1920 (the Pages panel is docked, no button): use `try`.
- Painted canvas text (not widgets) cannot be asserted with `expect`; take a picture.
- `try` steps always print "ok"; a failed `expect_*` inside is still a finding.
- Rebuild before reporting a text bug (stale embedded lessons).

Open findings and owners: B-07 Wire numbering, B-08 and B-09 Bench, B-10 Reports (undo after reopen), B-11 PLC, B-12 Standards.
