# CLI audit handoff
Branch claude/project-thread-271cdo. Code in crates/novo-cli/src/audit.rs, tests in crates/novo-cli/tests/audit.rs. Run: `cargo test -p novo-cli --test audit`.
Decisions: aliases are unique per pack, not across packs; warnings never fail unless `--strict`. QElectroTech is GPL, ideas only.
