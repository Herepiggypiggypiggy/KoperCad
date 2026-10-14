# Layout and branding: roadmap

Scope: how NovoElectric looks. That covers the dark theme, design tokens,
fonts, icons, the logo and app icon, branding, and the main window layout
and menu structure (co-designed with the workflow thread). The theme lives in
crate `novo-theme`; App shell (WS-01) wires it into `novo-app`, and Canvas
(WS-03) uses `CANVAS_DARK`.

Requested by Tadjiro. His direction: elegant rather than playful; no
N-shaped logo; not a copy of EPLAN; be critical; don't wait on him.

## Done
- Graphite theme with a single copper accent, Inter 4.1 and Phosphor icons,
  and WCAG contrast floors tested: `crates/novo-theme` (PR #20).
- Nova logo and app icon (`assets/branding`, re-rendered with
  `tools/brand-render`), with the Phase and Ground alternatives kept.
- `docs/design/ui-guidelines.md` (visual language only).
- Screen architecture (Workbench layout A, with Focus and Split as modes),
  after two critique rounds with workflow and the reviewer. Workflow folds
  it into docs/DESIGN.md.

## Next, in order
1. Drive PR #20 through review to merge. After the reviewer says READY,
   no more pushes to it.
2. Check workflow's docs/DESIGN.md PR against screen-architecture.md.
3. After #20 merges: a new PR from main with CanvasPalette entries for
   the DESIGN §5 marks (proposal, override, change colours).
4. Help App shell apply the theme (`novo_theme::apply`, dark only, the
   window icon from brand::ICON_PNG_256, `assets/branding/icon.ico` for
   exe and installer, and deleting `assets/icon`).
5. Build the real chrome widgets as the app reaches them: command field,
   outline, inspector card, dock and drawer.

## Open questions
- Structure board on first run: how much form, how much drawing.
- A light theme for bright rooms (dark stays the default).
- Does the dock moving aside during gestures feel calm in a real build?
