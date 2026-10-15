# Design system and aesthetics: handoff

Updated 2026-10-15 19:55 UTC. The thread is winding down (usage limit); a fresh session continues from here.

## State
- Repo herepiggypiggypiggy/kopercad, branch `claude/project-thread-mr5n3b` (the only branch pushed; restart it from main after each merge).
- #448 (spec) merged. #501 (novo-theme roles, light theme, density, widgets, gallery, `Palette::room`) is READY on a94c63b from the design reviewer and main reviewer B, undrafted, waiting for the integration lead. Do not push to it.
- Local only (not on GitHub): git branches `ds-pr3` (on top of #501: `border.control` commit 9c47a6b) and `ds-appearance` (the panel stack, based on the pre-merge spec). A copy is in the project folder: `design/design-system-stack/` (`stack.bundle`, patches, README). This handoff itself is only in the local stack until the next PR, so it is mirrored there too.

## The local stack, in PR order (cherry-pick from `ds-appearance`)
1. `border.control` role (3:1 outlines for checks, radios, buttons, fields), new gallery pictures (reviewer nit).
2. Preferences theme and density, `appearance.rs`, canvases follow the theme (`sync_appearance`), settings keys `appearance_theme` and `appearance_density`, env overrides `KOPERCAD_THEME` / `KOPERCAD_DENSITY`.
3. Start screen (`app/start.rs`).
4. Dock: wiring mode as a segmented track.
5. Drawers: underlined tabs, close icon, `widgets::table_head`.
6. Toasts (`widgets::toast`), command field icons per result kind, top bar Schematic/Cabinet segmented control.
7. Outline tabs, pin icon, Function/Location segmented (`segmented_hinted`), page rows (`row_sensing`), inspector (pin icon, relation rows, reset icon).
8. Live view colours from the palette (`potential_color(p, dark)`).
Check `git log main..ds-appearance` before cherry-picking; drop the first four doc commits (already merged).

## Resume
```
git fetch origin && git checkout -B claude/project-thread-mr5n3b origin/main
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test -p novo-theme -p novo-app
```
Screenshots: `scripts/screenshot.sh out.png -c 613,383 -k "c:108,118" -- target/debug/kopercad` (the `-c` click opens the example; steps in `-k` are keys, or `c:x,y` clicks; needs xvfb openbox xdotool imagemagick mesa-vulkan-drivers libxkbcommon-x11-0). Gallery: `cargo run -p novo-theme --example gallery`. Never `pkill -f` a pattern that matches your own shell.

## People
Roster: `coordination/threads.md`. Design reviewer cse_0114emQ5fSFrzcALbR4S18TU, main reviewer B session_01QSnXFBDRyvwwzhT1S1gFGN, integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S, 3D look session_017hfiGkLxw2PyCVFfnYgbdG (tell it when #501 merges), Workspaces session_01WAQSEGkMbyreCPG18tsSPc (shell, top bar, status bar: do not edit menu_bar.rs), Learning session_018gxMVUe63W8rvrNBwroP8s (shell #532, wants the guidance widgets), Canvas session_013KCL5cYRtSrq4yndv3Z7TD (gave me the canvas look items), Enclosure sizing session_014FSj7EkbsEgJrx8a1NWG9z (builds Cabinet side panel from my widgets).

## Decisions
- Colour roles, `tokens::*` stay as dark values. Density Compact 20, Standard 22, Touch 36. Light copper accent `#9A5427`, light wash `#F4E4D7` (accent on wash 4.5:1). Control outlines `border.control` 3:1.
- Physical 3D materials stay in view3d; room and overlays come from `Palette::room`.
