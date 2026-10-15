# Variants: handoff

Branch `claude/project-thread-uz8xho` of kopercad. Engines in `crates/novo-workflow/src/{expr,parametric,options,generate}.rs`;
UI in `crates/novo-app/src/app/variants.rs`; grid IO in `crates/novo-interop/src/sheets/spec.rs`.

State: all green locally (fmt, clippy -D warnings, workspace tests). Commands: project.options, project.variant,
project.generate, export.variant_pdf, export.spec (commands.rs, language.rs, workbench.rs).

Storage (no core change): project info properties `kc.options`, `kc.variants`, `kc.option.<id>`; object property
`kc.when`; block definition properties `kc.param.<name>`, `kc.repeat`, `kc.base`; macro template member `kc.if`.

Traps: a placed macro keeps no `kc.if` (stripped when instantiated); filtered definitions are named
`<macro>~<hash>`; `Mode::Replace` removes devices of blocks on generated pages (a device survives its symbols);
the starter specs live in `parametric::enrich_starter` until starter.rs carries them.
Resume: read docs/design/variants.md, run `cargo test -p novo-workflow -p novo-app --lib variants`.
