# Handoff: 3D panels and layers

Branch claude/project-thread-68eppp (PR A). Roadmap, audit and design: see ROADMAP.md, docs/design/3d-panels-audit.md, docs/design/visual-redesign-3d.md.
Code: crates/novo-app/src/app/view3d_options.rs (registry, `Owners::write`), view3d_controls.rs (toolbar edits a `Values` snapshot), view3d.rs (writes the diff back; heat gated by `heat_on`), workbench.rs (first match arm routes registry commands).
Resume: `cargo test -p novo-app --lib -- view3d_options view3d heat emc`; QA `scripts/qa.sh --only "3D: heat and EMC"`.
Traps: heat.on is the Cabinet's switch too; the Cabinet still has its own `cabinet.emc.on`; lessons anchor "view3d.heat" on the toolbar Heat button.
Talk to: designer thread (#1583), heat thread (heat_status.rs SimStatus), coordinator.
