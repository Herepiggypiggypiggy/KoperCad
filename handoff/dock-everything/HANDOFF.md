# Handoff: dock everything

Branch claude/project-thread-bb5jpo (kopercad). Started from main f90872a.

State: Dialog wrapper in `crates/novo-app/src/app/panel.rs` (`Dialog`, `dock_controls`, `MODAL_QUESTIONS`, `WIDE_DIALOG`); fold, floated list and default flag in `panel_dock.rs` (`set_folded`, `was_floated`, `set_dialogs_dock_by_default`). The app turns the default on in `app.rs` (`#[cfg(not(test))]`); unit tests of single windows keep floating windows, the new tests in `panel_dock::tests` turn it on.

Traps:
- Docking by default is off in tests on purpose: hundreds of tests read floating window rects.
- A dialog's dock name is `window:<title>`, a modal's is `modal:<name>`; dynamic titles dock per title.
- A docked modal answers like a click outside when its pane is closed.
- `Drive run` uses `.order(Foreground)` and so stays floating.
- QA scenarios were not rerun against docked windows (the harness runs the real app, so default docking is on there).

Resume: `cargo test -p novo-app --lib panel_dock`; fmt and clippy per CLAUDE.md.
