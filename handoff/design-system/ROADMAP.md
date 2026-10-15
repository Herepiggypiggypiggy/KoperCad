# Design system and aesthetics: roadmap

Scope: the visual quality of the whole app. Design system in `novo-theme`
(roles, type, space, radii, elevation, density, icons, states, light theme),
a consistency audit and migration of every panel and dialog, empty states,
and polish of the schematic canvas (line weights, selection and hover, grid,
page frame, title block). Spec: `docs/design/design-system.md`.

## Done
- Spec written and the delivery plan in section 9 of it.

## Next, in order
Follows section 9 of the spec: tokens and light theme, Preferences, icons,
panel migration PRs with before and after screenshots, canvas polish with the
canvas thread, lint test against raw values.

## Open questions
- Bundled monospace face (egui default today): worth a subset JetBrains Mono?
- Whether the canvas thread takes the `CANVAS_LIGHT` mapping or the design
  system thread does it in `novo-app/theme.rs`.
