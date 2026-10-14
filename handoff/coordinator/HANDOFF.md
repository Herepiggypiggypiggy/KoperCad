# Coordinator handoff

## Role
Project-chat Claude that starts and steers threads (session_018btnHEHYdedX4jueCivWjT). The integration lead in the "roadmap" thread (session_011PfTJ1QaP1JQJPwXcFnxBE) merges; the coordinator does not.
Duties: start threads on request (from Tadjiro or the integration lead, which has no start_thread_session); relay Tadjiro's messages by id to the relevant threads; retire finished threads; keep project instructions current; routinely review how the project runs and adjust it; stay quiet in the project chat unless something needs Tadjiro.

## Project
NovoElectric, github.com/Herepiggypiggypiggy/NovoElectric. Native Rust (egui/wgpu, SQLite) electrical CAD, fully local, Windows first.
Roadmap artifact: https://claude.ai/code/artifact/8ff48512-c76a-4566-8937-166b4b128d2e

## Tadjiro's standing rules
- No admin rights for anything (installs, builds, launchers).
- Don't wait on him. Never ask him to merge.
- It must be its own unique, elegant program, not an EPLAN copy. His EPLAN complaints are problems to avoid.
- Be critical.
- The coordinator routinely reviews how the project runs and adjusts it.
- Every thread keeps ROADMAP.md, HANDOFF.md and PROGRESS.md in handoff/<thread-slug>/ (see handoff/README.md).

## Threads (ids in coordination/threads.md)
- Roadmap / integration lead (merges).
- Wave 1: data model WS-02, app shell WS-01, canvas WS-03, symbol artwork WS-05.
- Project file WS-04: retired.
- Main reviewer; tester; layout/look/branding; workflow (owns docs/DESIGN.md).
- Phase 1: schematic editor WS-06, parts and properties WS-07, keyboard workflow WS-08, cross references WS-10, terminals WS-11.
- Design and workflow reviewer; release and builds.

## Decisions already made
- Wiring is pin-to-pin with automatic corners and T-junctions.
- One command bar replaces the menus and the insert centre.
- One model with five views.
- Window has no menu bar: an outline, a floating inspector and a bottom drawer.
- Approved (READY) PRs are frozen; further work goes in a new PR from main.
- Design comes first: feature threads build engines only until docs/DESIGN.md merges.

## Pending
- Retire the symbols thread (WS-05) after #37 merges.
- 60 fps GPU check on Tadjiro's PC via canvas-demo.exe in the experimental build.
- Logo pick: not blocking.
- Next milestone: end-to-end flow (open project, place a motor starter, wire, see terminals, save).

## How to resume
1. Read handoff/MAIN-HANDOFF.md, coordination/threads.md and this folder (ROADMAP.md, PROGRESS.md).
2. Call list_thread_sessions to see which threads are alive and what they are doing.
3. Call list_project_prs to see PR state; compare with threads.md.
4. Act on what needs doing (retire, start, relay); update PROGRESS.md and this file.

## Traps
- Only write in handoff/coordinator/ here; other sessions write nearby on the network mount.
- Don't route thread-to-thread messages; threads message each other directly (threads.md).
- Work on events, not polling loops.
