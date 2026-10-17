# Handoff: top bar and report designer

- PR #869 (merged; branch claude/project-thread-54v5a7): breadcrumb truncates to the room left (`top_bar_right`), outline button is an icon in Commissioning, Commissioning pins from 1100 pt (`PIN_COMMISSIONING_FROM`).
- PR #870 (merged; claude/project-thread-54v5a7-designer): title block rows sized to the form, `panel::grow` scales dialog defaults up to 1.5x, preview sample text is a number in narrow columns (PDF golden regenerated).
- BOM table headers (QA finding 9): main already carries an equivalent fix (`column_widths_min`, `header_min_widths`); #872 kept main's version and now only adds this handoff.
- Screenshots: /mnt/project-files/qa/shots-fix-topbar-designer/.
- Resume: `cargo test -p novo-app --lib menu_bar`, `cargo test -p novo-theme table`. Screenshot driver used Xvfb + openbox + xdotool + lvp Vulkan at 1366x768 and 1920x1080 (window sized with xdotool windowsize W H-20).
- Trap: disk fills on test builds; use CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0.
- Coordinator: session_012kLMYWW7HVrwCBXYG6vjqR.
