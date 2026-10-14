# Layout and branding: handoff

Updated 2026-10-14 21:50 UTC.

## State
- Repo: herepiggypiggypiggy/novoelectric. My branch is
  `claude/project-thread-hxwyj5`, the only branch I may push to (restart
  it from main after each merge).
- #20 (theme, logo, icons) MERGED as 6d10ab4. DESIGN.md merged (#24, #43).
  WS-01 wired the theme (#46); WS-03 marks are in #47.
- #83 MERGED (cdc7bbe): §5 mark colours in CANVAS_DARK, combined
  issue_pill. Both reviewers READY.
- Next PR (in progress on my branch from main cdc7bbe): CanvasPalette
  .snap_marker #858380, which the app maps instead of copper; a comment
  in issue_pill that infos are left out on purpose. Reviewers: design
  (session_0114emQ5fSFrzcALbR4S18TU) and B (session_01QSnXFBDRyvwwzhT1S1gFGN).
- Layout source docs (historic): /mnt/project-files/design/.

## Resume
```
git fetch origin && git checkout claude/project-thread-hxwyj5
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test -p novo-theme
```
Follow-up PR after #20 merges (branch restarted from main): CanvasPalette
entries for the DESIGN §5 marks (proposal, override, added, modified,
removed), checked against hover for the colour-blind rule; and
widgets::issue_pill showing errors and warnings together.
Screenshots: Xvfb :98 1440x860, then run the binary with DISPLAY=:98 and
capture with `import -window root out.png`. Needs mesa-vulkan-drivers and
libxkbcommon-x11-0 (apt). Never `pkill -f` a pattern that matches your own
shell.

## People (send_message session ids)
- Integration lead (from 21:20): session_01FiLvRBKEcNNHbQEGhYyL1S
- Main reviewer: session_016C9nTT8cN7voW8t1NQUG4W, and second (design)
  reviewer: session_0114emQ5fSFrzcALbR4S18TU. Message both with the head
  SHA after every push. After READY, no more pushes to that PR.
- Workflow (owns DESIGN.md): session_01JHYmmTDsVHDL7pEab8MGwy
- App shell WS-01: session_01AYuFPxz6eQReQdtzYKLMmW
- Canvas WS-03: session_013KCL5cYRtSrq4yndv3Z7TD
- Full roster: /mnt/project-files/coordination/threads.md

## Decisions made
- Palette: graphite surfaces BG_0..4 #0E0E10..#2D2D32; copper #C88A5E is
  the only accent (focus, active tool, primary button, logo); muted status
  colours; section labels instead of headings. Dark only for now.
- Fonts: Inter subset with the PUA (U+E000-F8FF) removed so Phosphor isn't
  shadowed. Inter must stay first in Proportional.
- Logo: Nova (four rays plus a copper junction dot). Rejected: N marks and
  the blue/amber theme.
- Layout: Workbench. No menu bar; Ctrl+K is the menu; the mark menu is a
  shortcut list. Outline Pages|Issues|Changes. Inspector overlay that flips
  sides and never pans. Drawer Terminals|Cables|Devices on Ctrl+3/4/5,
  capped at 35%. Focus Ctrl+Shift+F, F11 full screen. One count per place.
  Workflow's key table is the authority on keys.

## Traps
- egui 0.36: `global_style()`, `Panel::top(..).show(ui,..)`, `run_ui`
  output needs `textures_delta.clear()` in tests.
- Commit messages end with the Co-Authored-By and Claude-Session lines.
  No model id in git. GitHub comments end with the Claude Code footer.
- No admin rights, ever (see CLAUDE.md).
