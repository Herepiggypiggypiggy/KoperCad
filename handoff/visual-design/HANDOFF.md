# Visual design: handoff

Updated 2026-10-28. Branch `claude/project-thread-1ytuxa`, based on main cf49f8d.

## State
- Design doc `docs/design/visual-redesign.md` plus assets under `docs/design/visual-redesign/` (images, mockups, icons, tools). Docs only: no Rust changed.
- Boards also published as a design canvas (private until the owner shares it): https://claude.ai/artifact/2bcoRigCmcDmiFqdjyX5fo
- The boards are HTML, not egui output. Layout, counts, icons and behaviour are binding; pixel exactness is not.

## Resume
- Rebuild assets: see "Mockup sources" in the doc (`tools/koper_icons.py`, `tools/build_kit.py`, `tools/render.sh`). Needs `npm pack @phosphor-icons/core@2.1.1` and Playwright.
- Dark text tokens in the kit follow design-system.md exactly (`--tx2` #989691, `--tx3` #858380).

## Decisions already made
- Rail replaces the bottom dock and the text toolbars; undo and redo move to the top bar.
- One app menu behind the mark; menu names are a setting. Label policy 28 characters; menu budget 318 to 158 (target, held by a test in lane V2c).
- Own docking on `panel.rs`, no docking crate unless V1 asks the coordinator.
- Icons drawn on a 24 grid, stroke 1.5, round caps, shapes limited to what the egui painter draws.

## Traps
- direction.md count test and "toolbar buttons keep their words" are amended by the doc, section 2.
- Lessons name menus and dock tools: update anchors in the same PR as any menu or rail change.
