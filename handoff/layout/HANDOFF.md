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

## Cabinet layout (WS-15, Phase 3), state at 2026-10-15 19:55 UTC
Design: docs/design/cabinet-layout.md (objects, geometry rules, queries, commands, section 5a canvas controls).
Engine `crates/novo-layout`: `lib.rs` (Rect, Size, Placement, Plate, Rail, Duct, collision/outside/clearance/rail queries), `plan.rs` (place_all, place_all_below), `state.rs` (`CabinetLayout`: drop/move/rotate/remove, preview, accept, report), `model.rs` (load from the project, candidates, apply_plan, place/rotate/unplace commands). Tests in `crates/novo-layout/tests/`.
App: `crates/novo-app/src/app/cabinet.rs` (+ `cabinet_tests.rs`); canvas scene in `crates/novo-canvas/src/cabinet.rs`; colours in `crates/novo-app/src/theme.rs::cabinet_style`.

UI entry points (an engine without a UI entry is not done):
- Ctrl+2 / View > Cabinet opens the Cabinet canvas of a panel (location tag); panel chooser and Create cabinet in the header.
- Unplaced drawer on the left: drag a device onto the plate (landing-box ghost); drag a placed device to move it; R rotates and Delete removes the selection.
- Place all button: preview, Enter accepts as one undo step, Esc cancels.
- Ctrl+T or the header Table button: table face (Device, Size, X, Y, Turned, Rail, Problems); a row click selects the device on the plate.
- Hover on a device names the problem in words. Right-hand inspector and plate size: UI-integration thread (#443, cabinet_inspector.rs).
- Ducts (rails of kind Duct) are drawn as strips centred on their line.

PRs: merged #284, #287, #289, #372, #378, #403. READY at hand-off: #461 (ducts, 4ce992b), #472 (table polish, 2c2274a). Open: #444 (theme icons Reports/Commissioning/3D, ac046e5).
Not done: nobody has tried the cabinet drag by hand in the running app; ducts are not in collision/clearance checks; duct add/size/move tools and the routes overlay belong to the wire routing thread (new files, hook after #461); rules-based Place all (R7) is the Pro Panel thread (crates/novo-layout/src/rules.rs, then app wiring in cabinet.rs); the app recomputes load/candidates/report every frame (cache if it shows); local clippy flags novo-interop `Parsed` (not ours).
File ownership: I own app/cabinet.rs and cabinet_tests.rs; the UI-integration thread asks first before editing them beyond small hooks; I own Cabinet look and branding.
