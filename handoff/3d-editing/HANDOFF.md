# 3D editing handoff
State: nothing open. Every slice and both follow-ups are merged on main.
Code: `crates/novo-app/src/app/view3d_{measure,place,nudge,align,duplicate,wires,move,rail}.rs`, hooks in `view3d.rs`, commands in `novo-workflow` (`commands.rs`, `menus.rs`, `language.rs`) and `workbench.rs` (APP_COMMANDS and handler) plus the key filter in `app.rs`.
Traps: the scene only redraws when `scene.revision` changes, so rebuild through `novo_view3d::build::refresh`. The registration lists (commands, menus, language, workbench) collide on every 3D landing: after a merge, check `commands.rs` for a `cmd(` entry run into the next one (compile error "takes 5 arguments but 9") and `workbench.rs` for a missing `}` between match arms. Tests that touch the process-wide frame recorder (`register_adapter`, the Stats panel height) take `STATS_GUARD` in `view3d_move_tests.rs`. Arrow nudges only act while the pointer is over the view (tests send a PointerMoved first).
Rail card (`view3d_rail.rs`) is deliberately tight (row spacing 1 px) so it clears the Stats panel at 900x600; the margin there is about 8 px.
Headless screenshots: Xvfb, openbox, xdotool, `import`, LIBGL_ALWAYS_SOFTWARE=1; pictures in `/mnt/project-files/3d-editing/`.
Reviewers: R3 (UI), R5 (3D); leads A and B merge. See coordination/threads.md for ids.
