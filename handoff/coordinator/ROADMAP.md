# Coordinator roadmap

## Project
NovoElectric (github.com/Herepiggypiggypiggy/NovoElectric): a native Rust electrical CAD app (egui/wgpu, SQLite), fully local, Windows first.
Roadmap artifact: https://claude.ai/code/artifact/8ff48512-c76a-4566-8937-166b4b128d2e

## Scope
The coordinator is the project-chat Claude that starts and steers threads. The integration lead (thread "roadmap", session_011PfTJ1QaP1JQJPwXcFnxBE) merges PRs.
- Start threads on request, from Tadjiro or from the integration lead (it has no start_thread_session).
- Relay Tadjiro's messages by id to the relevant threads.
- Retire finished threads.
- Keep the project instructions current.
- Routinely review how the project runs and adjust it.
- Stay quiet in the project chat unless something needs Tadjiro.

## Done
- Started the roadmap/integration lead thread.
- Wave 1: data model (WS-02), app shell (WS-01), canvas (WS-03), symbol artwork (WS-05).
- Project file thread (WS-04 storage), now retired.
- Main reviewer, tester, layout/look/branding, workflow.
- Phase 1: schematic editor WS-06, parts and properties WS-07, fast keyboard workflow WS-08, cross references WS-10, terminals WS-11.
- Design and workflow reviewer; release and builds.

## Next, in order
1. Retire the symbols thread (WS-05) after #37 merges.
2. Get the 60 fps GPU check done on Tadjiro's PC (canvas-demo.exe in the experimental build).
3. Steer threads toward the next milestone: an end-to-end flow (open project, place a motor starter, wire, see terminals, save).
4. Logo pick: not blocking; raise with Tadjiro only when convenient.

## Open questions
- None blocking right now.
