# Design system and aesthetics: handoff

Updated 2026-10-15 19:20 UTC.

## State
- Repo herepiggypiggypiggy/kopercad, branch `claude/project-thread-mr5n3b`
  (the only branch I push to; restart it from main after each merge).
- #448 (spec) merged 19:13 UTC. PR 2 (this branch): tokens, light theme,
  widgets, gallery, plus the review fixes that landed after the merge.
- Next PRs, one at a time from main (local stack branch ds-appearance holds the
  work): Preferences theme/density and canvas restyle, start screen, dock,
  drawers, toasts, command field, top bar, outline, live view colours. Then
  inspector, dialogs, empty states, icons, canvas polish (with the canvas
  thread), learning widgets (novo-learn).

## Resume
```
git fetch origin && git checkout claude/project-thread-mr5n3b
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test -p novo-theme
```
Screenshots: `scripts/screenshot.sh out.png -c 613,383 -- target/debug/kopercad`
(needs `apt install xvfb openbox xdotool imagemagick mesa-vulkan-drivers
libxkbcommon-x11-0`). Never `pkill -f` a pattern that matches your own shell.

## People
Roster: `coordination/threads.md`. Related: Layout, look and branding
(cse_01UXbCeQW22y6tv74F2Xvbvm), 3D look (cse_017hfiGkLxw2PyCVFfnYgbdG),
Workspaces, menus and navigation (cse_01WAQSEGkMbyreCPG18tsSPc), App shell
(cse_01AYuFPxz6eQReQdtzYKLMmW), Canvas (cse_013KCL5cYRtSrq4yndv3Z7TD).

## Decisions
- Colour roles instead of numbered greys; `BG_0..4`, `TEXT`, `ACCENT` etc.
  keep working as the dark values so open PRs do not break.
- Density: Compact 20, Standard 22 (default), Touch 36.
- Light theme is first class, copper accent darkened to `#9A5427`.
