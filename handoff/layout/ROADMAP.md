# Layout and branding: roadmap

Scope: how NovoElectric looks. That covers the dark theme, design tokens,
fonts, icons, the logo and app icon, branding, and the main window layout
and menu structure (co-designed with the workflow thread). The theme lives in
crate `novo-theme`; App shell (WS-01) wires it into `novo-app`, and Canvas
(WS-03) uses `CANVAS_DARK`.

Requested by Tadjiro. His direction: elegant rather than playful; no
N-shaped logo; not a copy of EPLAN; be critical; don't wait on him.

## Done
- #83 merged (mark colours in palette, combined pill).
- #20 merged (theme, logo, icons); DESIGN.md merged; theme wired in #46.
- Graphite theme with a single copper accent, Inter 4.1 and Phosphor icons,
  and WCAG contrast floors tested: `crates/novo-theme` (PR #20).
- Nova logo and app icon (`assets/branding`, re-rendered with
  `tools/brand-render`), with the Phase and Ground alternatives kept.
- `docs/design/ui-guidelines.md` (visual language only).
- Screen architecture (Workbench layout A, with Focus and Split as modes),
  after two critique rounds with workflow and the reviewer. Workflow folds
  it into docs/DESIGN.md.

## Next, in order
1. Land #461, #472, #444 (the lead merges once READY and CI green).
2. Hands-on test of the cabinet drag and the table in the running app.
3. Duct checks in collision/clearance, with the routing thread's tools.
4. Support the design-system pass (#448): no raw colours in cabinet code.

## Open questions
- Structure board on first run: how much form, how much drawing.
- A light theme for bright rooms (dark stays the default).
- Does the dock moving aside during gestures feel calm in a real build?
