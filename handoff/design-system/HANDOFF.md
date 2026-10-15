# Design system and aesthetics: handoff

Updated 2026-10-15 19:20 UTC.

## State
- Repo herepiggypiggypiggy/kopercad, branch `claude/project-thread-mr5n3b`
  (the only branch I push to; restart it from main after each merge).
- PR 1 (#448: spec, `docs/design/design-system.md`, `scripts/screenshot.sh`, this
  folder): READY on 2338d9d, waiting for the integration lead to merge. Do not
  push to it.
- Local branches ds-tokens, ds-appearance hold the code PRs (stacked): tokens and
  light theme, Preferences, start screen, dock, drawers, toasts, command field,
  top bar. Split into small PRs from main once #448 is in. Still to do: outline
  flyout tabs, inspector, dialogs, live view colours, empty states, icons,
  canvas polish (with the canvas thread), learning widgets (after #456).
- Before screenshots of the app and preview example taken at main 074138a.

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
