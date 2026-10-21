# Claude integration: handoff

Branch: `claude/project-thread-c87c3y` (kopercad). First PR: crate `novo-ai` + app window.

How it fits
- `crates/novo-ai`: pure engine, tests never touch the network (`api::testing::Script` scripts replies).
  Real HTTPS is `UreqTransport` (ureq 3, rustls; added to `[workspace.dependencies]` as `ureq`).
- `crates/novo-app/src/app/assistant.rs`: window, background thread per turn (channel polled each
  frame), snapshot of the project filtered by `DataScope`, proposals applied through `NovoApp::execute`.
- Command `ai.assistant` in novo-workflow (commands, menus: Help, Dutch aliases).
- Settings: `Settings.ai` (`AiSettings`); key in `%LOCALAPPDATA%\KoperCAD\ai\api-key` (not encrypted,
  said so in the UI); usage ledger `usage.json` beside it.

Rules baked in
- Model ids and rules from the claude-api skill: default `claude-opus-5-5`, adaptive thinking, effort
  low/medium/high, no sampling params, no forced tool_choice, thinking blocks returned untouched,
  server-side refusal fallback beta on models that support it.
- Claude never edits directly; `propose_edit` records a card; Apply = one command = one undo step.
- Project text is data (system prompt says so). Scope None = tools answer "not allowed".
- Costs are list-price estimates, labelled so.

Not verified: a real call to api.anthropic.com (no key in the cloud session). First thing for
anyone with a key: Settings > Check key, then one question, and compare the cost line with the
Anthropic console.

Resume: `cargo test -p novo-ai`, `cargo test -p novo-app assistant`.
