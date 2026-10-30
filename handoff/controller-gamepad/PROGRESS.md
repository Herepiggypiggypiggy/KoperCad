2026-10-30 UTC lane C5 started: plan read, `gilrs` chosen for Windows (needs libudev on Linux, so cfg(windows) only), pure `novo_controller::gamepad` written with 79 hand-written tests.
2026-10-30 UTC host service (fake device plus gilrs reader), Controller card Gamepad section, map editor, calibration wizard, control command `pad` and QA step added.
2026-10-30 UTC PR #1795 opened from claude/controller-c5-gamepad (head 4468c86c); clippy, tests and QA 1590/1590b green on Linux; screenshots in /mnt/project-files/qa/pr1795-4468c86c/. Not run: Windows hardware, wasm build.
