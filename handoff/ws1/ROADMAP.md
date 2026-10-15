# WS-01 App shell: roadmap

Scope: `crates/novo-app`, the native window (eframe/egui 0.36, wgpu). It wires the other crates together: storage, canvas, editor, workflow, terminals and theme. It also owns the Windows installer (`packaging/windows/novoelectric.iss`, per user, no admin), `.github/workflows/package-windows.yml` and the CI pieces for the app. Hard rule: nothing may ever need admin rights.

## Done
- PR #3 merged (11363c1): shell with menus, panels, status bar, Ctrl+K palette, actions and shortcuts, unsaved-changes prompts, settings persistence, per-user Inno installer, static CRT, installer CI that installs as a standard user.
- PR #34 open: storage wiring (open, save, save as, autosave, crash recovery, untitled projects kept on disk) and the canvas (novo-canvas `ModelCanvas` draws the open page).

## Next, in order
1. Get #34 green and merged. Answer the reviewer (cse_016C9nTT8cN7voW8t1NQUG4W).
2. Theme, once #20 merges: `novo_theme::apply`, dark only, remove the theme actions, use the Nova icon from assets/branding, put the dark palette on the canvas, use section_label and primary_button. Ready locally, see HANDOFF.
3. **Gate 1** (integration lead): in the real app, create or open a project, place a motor starter, wire it, see its terminals, save and reload.
   - Host the novo-editor `SchematicView` (#38) behind the `CanvasView` trait, replacing `ModelCanvas`. Forward the editor's ChangeSets to `backend.record`. Each frame, call `take_page_request()` and switch to that page.
   - Host the WS-08 command field (#29). WS-08 will wire `palette.rs` to `novo_workflow::palette::Palette::search` and shortcuts to `Keymap::resolve` themselves.
   - Host terminals (#30, WS-11).
   - Cross references (#31, WS-10): G, Shift+G, Alt+Left/Right through `novo_xref::Navigator`.
   - Coordinate the scenario test with the tester (cse_01LMMcDMt9qaHXoSXy7iDu58).
4. Reshape the chrome to docs/DESIGN.md once it lands: one command bar instead of menus, five views (Schematic, Connections, Devices, Cabinet, Reports). Until then, keep the UI thin and add no new panels.
5. Later: properties via `novo_parts::PropertySheet` (WS-07 #36), symbol artwork from novo-symbols in the canvas (currently PlaceholderArt).

## Follow-ups from the #46 reviews (non-blocking, not started)
From the main review (5523068560):
- Quitting while writes fail exits silently (`on_exit` only logs). Consider a non-modal "Couldn't save. Save to another place?" shown only while writing is failing.
- An untitled project without a session (no `data_dir()`, or `start_untitled` failed) is dropped on Close, New or Quit with only a `log::warn`. Tell the user it is not protected.
- `kept-untitled-*` files can't be deleted from the UI, so the UNSAVED list grows without limit.
- The save dialog always suggests `Untitled.novo`; suggest the project name.
- Move the `ron` dev-dependency into `[workspace.dependencies]`.
- DESIGN §4: I and a tapped Space should open the command surface; `RESERVED_SHORTCUTS` still labels I as "insert symbol"; "Open example" on the start panel.

From the design review (5523325325):
- New project flow (§3, §6): "New project, type a name, Enter", written straight to `Documents\NovoElectric\<name>.novo` with no file dialog; keep Save as for the exception. Drop the permanent "Not saved yet" label for untitled work that is safe on disk.
- Recovery modal: restore automatically with a toast offering the file version instead of the Restore/Discard modal (no Escape today). Drop "Save to keep it" for projects that already have a file.
- Snap marker is copper (`theme.rs` `snap_marker: color(p.selection)`); §5 says copper is never a mark. Use the hover steel or `TEXT_FAINT`.
- The mark is the text "NovoElectric", not the Nova mark from novo-theme; its menu should follow §3 (New, Open, Recent, Export PDF, Print, Project settings, Preferences, Quit) rather than lead with Save.
- Focus mode hides the pinned outline too (`pinned = outline_pinned() && !focus_mode`), which moves the drawing; §3 says it hides only unpinned panels. Keep pinned panels or ask the workflow thread to change §3.
- Escape on the flyout uses `key_pressed`, not consume, so the same press also runs the canvas Escape ladder and clears the selection.
- With nothing open, "No project open" shows twice and Schematic shows as selected.
- `describe_error`'s doc comment still says "for the status bar".

## Open questions
- Resolved: WS-06's `SchematicView::show` now returns the frame's merged ChangeSet (#38, 81c0caa).
- Panels and search (named panels, tag search, Go to all references) were built locally and then shelved, because DESIGN.md decides the layout. See HANDOFF.

## Next (2026-10-15)
- Owner hands-on check of the place wheel: docs/HANDS-ON-WHEEL.md.
- Ctrl+, for Preferences, `resolve_while_wiring` in the shell, `link` pick-flow preview, `xref.link` toast wording, Enter-path test for New... potential. Details in HANDOFF "Current state".
