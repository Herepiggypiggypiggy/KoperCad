# Handoff: device descriptions by recipe

Branch `claude/project-thread-904jsh`. Engine in `crates/novo-parts/src/describe.rs`, tests in its `tests` module and `crates/novo-parts/tests/describe.rs`. App glue in `crates/novo-app/src/app/describe_recipes.rs`, command `parts.describe` registered in novo-workflow (commands, menus, language), novo-theme presentation and workbench.

Resume: `cargo test -p novo-parts describe; cargo test -p novo-parts --test describe`.

Decisions: an unstated contact count is left out, never written as 0; the kind comes from `part_role` (function, keywords, description); a kind with no recipe keeps the maker's description. Generic recipes only; no shop data in the repository.
