# Handoff: pages, groups and panels

Ask: make new pages, page groups and panels from the pages navigator. Pages in the same panel name under different page groups (`=10D41+K1B1G1`, `=11D21+K1B1G1`) are one panel, so one cabinet layout.

State: no new entity. `Page::structure` already holds group and panel. Added `Command::SetPageStructure`, `structure` on `Command::AddPage`, `Project::panels()`, `page_groups()`, `next_page_name()`. Tests: `crates/novo-core/tests/panels.rs`.
Next: outline UI (new page/group/panel, move page), coordinate with Layout and App shell threads; interruption points (Potentials thread) use page names with the structure prefix.

UI (PR 2): crates/novo-app/src/app/pages.rs (form + form_command, tested) and the outline in app.rs (structure_headers, move_page, PageView). Spec: DESIGN PR #238 section Pages navigator. Not done: Cabinet canvas per panel (Layout thread), page-name rules for inserted pages (section 3).
