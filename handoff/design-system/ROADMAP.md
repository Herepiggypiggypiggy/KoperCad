# Design system and aesthetics: roadmap

Scope: the visual quality of the whole app. Design system in `novo-theme`
(roles, type, space, radii, elevation, density, icons, states, light theme),
a consistency audit and migration of every panel and dialog, empty states,
and polish of the schematic canvas (line weights, selection and hover, grid,
page frame, title block). Spec: `docs/design/design-system.md`.

## Done
- Spec merged (#448).
- #501: roles, light theme, density, widgets, gallery, `Palette::room` (3D view). READY on a94c63b, waiting for merge.
- Built locally, not yet pushed (branch `ds-pr3` and `ds-appearance`): `border.control`, Preferences theme and density with canvas restyle, start screen, dock wiring track, drawer tabs and table heads, toast widget, command field icons, top bar segmented switch, outline tabs and rows, inspector, live view colours.

## Next, in order
1. After #501 merges: restart `claude/project-thread-mr5n3b` from main and push the local work as small PRs, one at a time (list in HANDOFF.md), each with before and after screenshots in dark and light.
2. `segmented_icons(ui, current, &[(id, icon, long, short)], compact) -> Option<id>` with a per-option disabled reason (Workspaces thread wants it for the switcher; do not touch menu_bar.rs).
3. Guidance widgets in `novo_theme::guidance`: `pointer(ctx, rect, pulsing)`, `note(ctx, rect, number, text)`, `lesson_card(ui, header, body, buttons) -> Option<CardAction>` (CardAction: Back, ShowMe, Skip, Next, Restart, Close, Open(String)), `lesson_link(ui, title) -> bool`. Learning thread then switches learn_ui.rs to them.
4. Canvas look (canvas thread agreed I own it; wait for #339 before touching the handle loop in widget.rs): sheet vs surround, frame lines 3:1, title-block caption/value/zone colours and larger text, selection halo and hover, grid dot fade.
5. Remaining dialogs, empty states per spec section 6, icon sizes and fill weight, lint test against raw colours, FontId and literal radii in `novo-app`.

## Open questions
- Bundled monospace face (egui default today): worth a subset JetBrains Mono?
- The 3D look thread will tell us when `Palette::room` is wired (`RoomColors` in view3d).
