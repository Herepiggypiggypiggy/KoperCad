# Roadmap summary

Full roadmap with dependencies and done-criteria:
https://claude.ai/code/artifact/8ff48512-c76a-4566-8937-166b4b128d2e

State as of 2026-10-17, main `3bfd02d`. "In the app" means merged on main and reachable from the program.
The plan for the first test release is `handoff/release-0-1/` (the blocker list and exit criteria are in
`release/v0.1-plan.md` on the project mount); what the release contains is in `RELEASE_NOTES.md`.

## Phases

| Phase | Gate at the end | State |
| --- | --- | --- |
| 0 Foundation | Model API v0 frozen, canvas draws it | Done |
| 1 Schematics | Motor starter drawn over 2 pages, saved, printed to PDF | Done |
| 2 Intelligence | Terminal, cable and report pages generate live | Done; terminals, cables, reports, checks, numbering, CLI and Python are in the app or available |
| 3 Physical design | Cabinet laid out, wires routed, drawing set out | In the app: cabinet plate, rails, ducts, place all, enclosure sizing, door, wire routing and lengths, 3D view, PLC and I/O, revisions with compare |
| 4 Simulation | Star-delta starter simulates and animates | Live view with coils, contacts, timers, lamps and motors; motor start estimate; fault scenarios; heat and airflow model. Device models for more parts continue |
| 5 AI and interop | | DXF and STEP import, DXF export; Dutch reports; Learn lessons; commissioning and field mode. Not started: AI assistant. Paused: EPLAN connectivity and PLC program export |

## Release 0.1.0 beta (now)

The aim is one complete version that test users can install without administrator rights, tested on Windows,
with the known limitations written down. Until the tag, only these change on main: fixes for a release blocker,
fixes for a bug found by a tester scenario, docs and tests. New features wait for 0.2.

The blockers (summary, owners are in the plan):

- Open pull requests landed and rebased (numbering widgets, QA fixes for the motor starter, 3D wire colours,
  report pages, terminal positions).
- Crash and problem log with Help > Report a problem.
- Save-file compatibility: older builds' files open, newer files are refused untouched, autosave recovery works.
- Release packaging: installer, portable zip and checksums on a GitHub pre-release.
- Per-user install, upgrade and uninstall checked on a standard Windows user.
- Windows hands-on pass: wheel timing, live view, scenarios A to J.
- The motor-starter bench path: wire numbers across an overload relay, cut list completeness, export messages.
- Part suggestions that fit the device; readable text at fit zoom; no command without a handler.
- Honest labels on every estimate, and a Help page "About the numbers".
- Start-up safety on machines with no usable GPU, remote desktop and scaled displays.
- Tester README, release notes and third-party notices (this document set).

## After 0.1 (0.1.x fixes, then 0.2)

- Mouse-drag section plane in 3D; heat arrow hover values; autosave interval; custom symbol folder; terminal jumper
  number choice and "accept all" proposals; read-only open.
- Power and control wire colours (needs a circuit class); motor starter sizing in the app; macro parameter authoring.
- Installer auto-update and code signing (removes the SmartScreen warning).
- Dutch for the whole program.
- Single-line and fluid diagrams.
- Verified ampacity and ISO 13849-1 tables, a validated 3D airflow result.
- Multi-user editing.
- AI assistant that edits through the same command path as the user.
- EPLAN connectivity and PLC program export, after the pause ends.

## Where to look

- `docs/DESIGN.md` and `docs/design/`: how the program is meant to work.
- `docs/WINDOWS-CHECK.md`: what only a person on a Windows PC can confirm.
- `docs/TEST_REPORT.md`: the tester's rounds.
- `docs/thermal-validation.md`, `docs/PROTECTION.md`, `docs/functional-safety.md`: what the models are and how far to trust them.
- `handoff/MAIN-HANDOFF.md` and `handoff/<thread>/`: the state of every workstream.
