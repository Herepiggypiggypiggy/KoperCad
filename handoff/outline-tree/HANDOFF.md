# Handoff: outline tree

Branch claude/project-thread-siv4ut (repo kopercad). Ask from Tadjiro: left nav unusable (no right click on empty space, empty menus, grouping unclear, cabinets need a naming convention). Fix and redesign.

State: see docs/design/outline-tree.md. Code: crates/novo-app/src/app/outline_tree.rs (pure), outline.rs (egui), outline_tests.rs. Old structure_headers/page_row removed from app.rs; PageForm kept only for "Several pages...".

Resume: `cargo test -p novo-app outline`, `cargo clippy -p novo-app --all-targets -- -D warnings`. Row rects are recorded under cfg(test) so tests can click rows.

Traps:
- Keys: `outline_keys` runs before the app's key commands and consumes F2/Delete/Ctrl+D/arrows only when the tree has the last click and no text box has focus.
- A structure node made only for a cabinet's plate must not show twice (tree::build drops loose nodes that appear under a group).
- `Reveal` selects and scrolls to a row after an edit; it resolves on the next frame because the tree is rebuilt.
- main had a duplicated clippy attribute in novo-interop/src/sheets/fields.rs; removed in this PR.
- Cabinet enclosure choice is stored as structure property `enclosure` (tree::ENCLOSURE_PROPERTY).
- Not tested in a real window.
