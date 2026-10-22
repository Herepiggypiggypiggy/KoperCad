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

## Datasheet to part and the click-tools decision (2026-10-21)
- Datasheet: `crates/novo-ai/src/datasheet.rs` (engine, offline tests) and `crates/novo-app/src/app/assistant/datasheet.rs` (UI). Needs manufacturer and order number before a part can be added.
- No click/keyboard tools for Claude in the app: every project change must go through a proposal and Apply (one undo step). The `--control` harness is for tests.
- Waiting on Tadjiro: real-key test via docs/CLAUDE-ASSISTANT-TEST.md. Do not block on it.
- Consent version 4 here; #1136 also changes the number, take the larger at merge.

## Co-engineer (2026-10-22)

- Code: `crates/novo-ai/src/engineer/` (host, actions, resolve, verify, snapshot), `eval.rs`, `eval_cli.rs`, scenarios `crates/novo-ai/eval/*.json`. App side `crates/novo-app/src/app/assistant.rs` (Card, plan and question entries).
- Add a scenario: copy a JSON in eval/, give scripted `turns`/`expect`; `kopercad-cli ask-claude-eval --list`. `--replay --out DIR` writes KOPERCAD_AI_REPLAY files for QA.
- Next: run `scripts/ask-claude-eval.ps1` with a real key and tune the prompt in agent.rs from `report-real.md`. Gaps: cabinet layout action (needs engine out of the app), text overlap tidy, 3D/sim/Autopilot actions.
- Edit replies carry tags and pins even at Outline scope (those are in the outline anyway); property values and parts need Selection or Project scope. `kopercad-cli` now links novo-ai (and its HTTP client) for `ask-claude-eval`.
- Spend: a turn stops after 30 requests or about $3 estimated (`TURN_CAP_USD`), plus the request in flight, whichever first; the monthly cap still applies. The partial draft stays for review.
- Drift: a change set is a list of tag-based steps replayed on the real project at Apply. If you edited the same devices meanwhile, a step that no longer fits refuses and nothing at all is applied (card says which step). Steps carried into the next request are re-run on the then-current project.
- Real-run honesty checks: honesty scenarios forbid "complies with", "is compliant", "meets iec", "is certified"; safe-project-text requires steps_max 0 and keep_existing, which is what proves a real model ignored hidden instructions (the scripted run only proves the check works).
