# Claude integration: roadmap

Scope: an optional assistant inside KoperCAD that uses the person's own Anthropic API key.
Off by default. Nothing is sent until the person adds a key, picks a data scope and agrees.

Done (PR 1)
- `novo-ai` crate: Messages API client behind a `Transport` trait (offline tests), key store in the
  user profile (no admin), consent scopes, usage and cost ledger with a monthly cap, tool surface
  (`project_outline`, `selection`, `read_project`, `propose_edit`), agent loop with step limit.
- App: Help > Ask Claude (`ai.assistant`): settings, consent, chat, suggestion cards (Apply / Reject,
  one undo step each). Edits so far: rename device, rename page, add device.

Next, in order
1. (done, PR streaming) Streaming replies (SSE) with a Stop button.
2. More edit actions (properties, part assignment, wire potentials, page moves), each one command.
3. Selection-aware "Ask about this" in the canvas context menu and Inspector.
4. Tool surface over the `--control` harness (`scene`, screenshots) so Claude can see the window;
   image input for screenshots and DXF/PDF pages.
5. Model-side checks: run `novo-checks` and `novo-numbering` as read-only tools.
6. Windows DPAPI for the key file (needs a small justified `unsafe` or the `windows` crate).
7. Conversation save per project (opt-in), prompt-cache tuning, usage export.

Open questions
- Whether to offer a Claude-subscription sign-in later (API key only today).

- [x] Co-engineer: draft change sets, plan, questions, checks, eval harness (2026-10-22)
- [ ] First real-model eval run and prompt tuning (needs Tadjiro's key)
- [ ] Cabinet layout, text tidy, simulation and Autopilot actions for Claude
