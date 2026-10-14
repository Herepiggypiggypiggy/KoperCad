# Layout and branding: handoff

Updated 2026-10-14 17:06 UTC.

## State
- Repo: herepiggypiggypiggy/novoelectric. My branch is
  `claude/project-thread-hxwyj5`, the only branch I may push to.
- PR #20 "Layout and branding: novo-theme graphite theme, icons, Nova
  logo and app icon" is open, not a draft. Head is 2d60fdd. That commit
  answers the second reviewer's (session_0114emQ5fSFrzcALbR4S18TU) four
  required items: the preview is now the Workbench window, icons are
  grouped action/tool/canvas/panel/mark/status, the chrome wording is
  updated, and the copper rule is fixed. Design reviewer says READY,
  so #20 is FROZEN: never push to it again. CI on 2d60fdd running.
- #20 must stay theme only (integration lead). Its body is updated.
- Layout docs are not in git: /mnt/project-files/design/
  screen-architecture.md, layout-a/b/c.png. Workflow owns docs/DESIGN.md
  and is folding them in. Draft: /mnt/project-files/design/DESIGN-draft.md.
  I said ok with three fixes: dock "moves aside", not "fade"; add the
  sizes table; HUD 4.5:1 plus a light-theme open question.
- examples/preview.rs is now the Workbench (variant A of the old concept).
  segment, issue_pill and field are in #20; rail_button was dropped.
  /mnt/project-files/design/code/concept.rs still has the B and C
  variants, needed only to re-shoot layout-b/c.png.

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
- Integration lead / coordinator: session_011PfTJ1QaP1JQJPwXcFnxBE
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
