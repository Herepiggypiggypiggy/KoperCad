# Cabinet toolbar cleanup: handoff

Branch `claude/project-thread-zxmg7r` in kopercad. Code: `crates/novo-app/src/app/cabinet.rs` (bar, status row, empty state), `cabinet_bar.rs` (`Fold`, widths, pill), `heat.rs` (legend total, badges).
Pictures: `docs/design/cabinet-toolbar-cleanup/` (before from the 2026-10-16 audit, after from `target/debug/kopercad` in Xvfb with the example project).
Resume: `cargo test -p novo-app`, then re-shoot with Xvfb + openbox + xdotool (window resized to the screen size).
Reviewers: R3 cse_01SL6nnpsSpN8Tjt3tfo8K5V, design reviewer B cse_01WP56F6BoXwpQhqa2zpkYab, design lead cse_01GM9Kk2Z3xm9K7188nmQRow.
Not verified: Windows font rendering, 125/150 percent DPI, GPU colour.
